//! Catalogue des applications : `apps.json` à la racine du dépôt FCDI-Apps.
//! La version en ligne est lue à chaque vérification ; sinon on retombe sur la copie en cache,
//! puis sur celle intégrée au lanceur.

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub const CATALOG_URL: &str = "https://raw.githubusercontent.com/Cfrancky77/FCDI-Apps/main/apps.json";
const EMBEDDED: &str = include_str!("../apps.json");

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Catalog {
    pub apps: Vec<AppEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppEntry {
    /// Identifiant stable (sert de clé pour l'installation).
    pub id: String,
    /// Première partie du nom, en blanc (ex. « Luminux »).
    pub name: String,
    /// Seconde partie du nom, en couleur, collée au nom (ex. « Craft » ou « IA » avec une espace devant).
    #[serde(default)]
    pub accent: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub description_en: String,
    #[serde(default)]
    pub publisher: String,
    /// Dépôt GitHub « propriétaire/nom » qui publie les Releases.
    pub repo: String,
    /// Fin du nom du fichier à télécharger dans la Release (ex. « x86_64.AppImage »).
    pub asset: String,
    /// Adresse d'une icône PNG.
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub website: String,
    /// Couleur d'accent « #RRGGBB ».
    #[serde(default)]
    pub color: String,
}

impl AppEntry {
    pub fn full_name(&self) -> String {
        format!("{}{}", self.name, self.accent).trim().to_string()
    }
    pub fn repo_url(&self) -> String {
        format!("https://github.com/{}", self.repo)
    }
}

pub fn embedded() -> Catalog {
    sanitize(serde_json::from_str(EMBEDDED).expect("apps.json intégré invalide"))
}

// ---------------------------------------------------------------------------------------------
// Durcissement : le catalogue vient d'Internet, on n'en garde que ce qui est sûr.

fn clean(s: &str) -> String {
    // Pas de retour à la ligne ni de caractère de contrôle (injection dans les raccourcis du bureau).
    s.chars().filter(|c| !c.is_control()).collect::<String>().trim_end().to_string()
}

fn valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 48 && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn valid_repo(s: &str) -> bool {
    let part = |p: &str| {
        !p.is_empty() && p != "." && p != ".." && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    matches!(s.split_once('/'), Some((o, r)) if part(o) && part(r))
}

fn https_url(s: &str) -> Option<&str> {
    (s.starts_with("https://") && !s.chars().any(|c| c.is_control() || c.is_whitespace())).then_some(s)
}

fn icon_url(s: &str) -> bool {
    const HOSTS: &[&str] = &["raw.githubusercontent.com", "github.com", "avatars.githubusercontent.com", "user-images.githubusercontent.com"];
    https_url(s)
        .and_then(|u| u.strip_prefix("https://"))
        .and_then(|u| u.split('/').next())
        .is_some_and(|h| HOSTS.contains(&h))
}

fn valid_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Écarte les entrées invalides et nettoie les textes.
pub fn sanitize(cat: Catalog) -> Catalog {
    let mut seen = std::collections::HashSet::new();
    let apps = cat
        .apps
        .into_iter()
        .filter(|a| valid_id(&a.id) && valid_repo(&a.repo) && !a.asset.is_empty() && !a.asset.contains(['/', '\\']))
        .filter(|a| seen.insert(a.id.clone()))
        .map(|mut a| {
            a.name = clean(&a.name);
            a.accent = clean(&a.accent);
            a.description = clean(&a.description);
            a.description_en = clean(&a.description_en);
            a.publisher = clean(&a.publisher);
            if !icon_url(&a.icon) {
                a.icon.clear();
            }
            if https_url(&a.website).is_none() {
                a.website.clear();
            }
            if !valid_color(&a.color) {
                a.color.clear();
            }
            a
        })
        .collect();
    Catalog { apps }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_integre_valide() {
        let before: Catalog = serde_json::from_str(EMBEDDED).unwrap();
        assert_eq!(before.apps.len(), embedded().apps.len(), "une entrée du catalogue est rejetée");
    }

    #[test]
    fn entrees_dangereuses() {
        let json = r##"{"apps":[
          {"id":"ok","name":"Ok","repo":"Cfrancky77/ok","asset":"x86_64.AppImage","icon":"https://raw.githubusercontent.com/a/b/c.png","website":"https://example.org","color":"#FFAA00"},
          {"id":"../etc","name":"x","repo":"a/b","asset":"x"},
          {"id":"bad-repo","name":"x","repo":"a/../b","asset":"x"},
          {"id":"bad-asset","name":"x","repo":"a/b","asset":"../x"},
          {"id":"ok","name":"doublon","repo":"a/b","asset":"x"},
          {"id":"inj","name":"x","repo":"a/b","asset":"x","description":"gentil\nExec=/bin/evil","icon":"http://evil.example/i.png","website":"file:///etc/passwd","color":"rouge"}
        ]}"##;
        let cat = sanitize(serde_json::from_str(json).unwrap());
        let ids: Vec<_> = cat.apps.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["ok", "inj"]);
        let inj = &cat.apps[1];
        assert!(!inj.description.contains('\n'));
        assert_eq!((inj.icon.as_str(), inj.website.as_str(), inj.color.as_str()), ("", "", ""));
        assert_eq!(cat.apps[0].color, "#FFAA00");
    }
}

fn cache_path() -> Option<std::path::PathBuf> {
    dirs::cache_dir().map(|d| d.join("fcdi-apps").join("apps.json"))
}

pub fn cached() -> Option<Catalog> {
    let text = std::fs::read_to_string(cache_path()?).ok()?;
    serde_json::from_str(&text).ok().map(sanitize)
}

/// Lit le catalogue en ligne et le garde en cache.
pub fn fetch() -> Result<Catalog> {
    let text = crate::net::get_string(CATALOG_URL)?;
    let cat: Catalog = sanitize(serde_json::from_str(&text)?);
    if let Some(p) = cache_path() {
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(p, &text);
    }
    Ok(cat)
}
