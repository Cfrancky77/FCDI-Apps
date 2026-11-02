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
    serde_json::from_str(EMBEDDED).expect("apps.json intégré invalide")
}

fn cache_path() -> Option<std::path::PathBuf> {
    dirs::cache_dir().map(|d| d.join("fcdi-apps").join("apps.json"))
}

pub fn cached() -> Option<Catalog> {
    let text = std::fs::read_to_string(cache_path()?).ok()?;
    serde_json::from_str(&text).ok()
}

/// Lit le catalogue en ligne et le garde en cache.
pub fn fetch() -> Result<Catalog> {
    let text = crate::net::get_string(CATALOG_URL)?;
    let cat: Catalog = serde_json::from_str(&text)?;
    if let Some(p) = cache_path() {
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(p, &text);
    }
    Ok(cat)
}
