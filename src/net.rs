//! Accès réseau : Releases GitHub et téléchargements.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::io::{Read, Write};
use std::path::Path;

const UA: &str = concat!("FCDI-Apps/", env!("CARGO_PKG_VERSION"));

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(std::time::Duration::from_secs(15)))
        .http_status_as_error(false)
        // Certificats du système (comme le navigateur), utile derrière un proxy d'entreprise.
        .tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
        .build()
        .into()
}

fn request(url: &str) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
    let mut req = agent().get(url).header("User-Agent", UA);
    // Jeton facultatif pour dépasser la limite de 60 requêtes par heure de l'API GitHub.
    if url.starts_with("https://api.github.com/") {
        req = req.header("Accept", "application/vnd.github+json");
        if let Ok(tok) = std::env::var("GITHUB_TOKEN") {
            if !tok.is_empty() {
                req = req.header("Authorization", &format!("Bearer {tok}"));
            }
        }
    }
    req
}

pub fn get_string(url: &str) -> Result<String> {
    let mut resp = request(url).call().with_context(|| format!("connexion à {url}"))?;
    let status = resp.status();
    let body = resp.body_mut().read_to_string()?;
    if !status.is_success() {
        if status.as_u16() == 403 && body.contains("rate limit") {
            bail!("limite de l'API GitHub atteinte, réessayez dans une heure");
        }
        bail!("{url} : HTTP {}", status.as_u16());
    }
    Ok(body)
}

#[derive(Clone, Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
}

impl Release {
    pub fn version(&self) -> String {
        self.tag_name.trim_start_matches(['v', 'V']).to_string()
    }
    pub fn asset_matching(&self, suffix: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name.ends_with(suffix))
    }
}

/// Releases publiées d'un dépôt, de la plus récente à la plus ancienne.
pub fn releases(repo: &str) -> Result<Vec<Release>> {
    let text = get_string(&format!("https://api.github.com/repos/{repo}/releases?per_page=50"))?;
    let mut list: Vec<Release> = serde_json::from_str(&text)?;
    list.retain(|r| !r.draft);
    Ok(list)
}

/// Télécharge `url` vers `dest` (via un fichier `.part`), en appelant `progress(fait, total)`.
pub fn download(url: &str, dest: &Path, mut progress: impl FnMut(u64, u64)) -> Result<()> {
    let mut resp = request(url).call().with_context(|| format!("connexion à {url}"))?;
    if !resp.status().is_success() {
        bail!("téléchargement : HTTP {}", resp.status().as_u16());
    }
    let total = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let part = dest.with_extension("part");
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("création de {}", dir.display()))?;
    }
    let mut file = std::fs::File::create(&part).with_context(|| format!("écriture de {}", part.display()))?;
    let mut reader = resp.body_mut().with_config().limit(u64::MAX).reader();
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        done += n as u64;
        progress(done, total);
    }
    file.sync_all()?;
    drop(file);
    if total > 0 && done != total {
        let _ = std::fs::remove_file(&part);
        bail!("téléchargement incomplet ({done} / {total} octets)");
    }
    std::fs::rename(&part, dest)?;
    Ok(())
}

/// Compare deux versions « 1.2.3 », « 0.4.0-beta.2 »… (numérique, partie par partie).
pub fn cmp_versions(a: &str, b: &str) -> std::cmp::Ordering {
    fn parts(v: &str) -> (Vec<u64>, bool) {
        let v = v.trim_start_matches(['v', 'V']);
        let (main, pre) = match v.split_once('-') {
            Some((m, _)) => (m, true),
            None => (v, false),
        };
        (main.split('.').map(|p| p.parse().unwrap_or(0)).collect(), pre)
    }
    let (pa, prea) = parts(a);
    let (pb, preb) = parts(b);
    let n = pa.len().max(pb.len());
    for i in 0..n {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        if x != y {
            return x.cmp(&y);
        }
    }
    // Une préversion passe avant la version finale du même numéro.
    match (prea, preb) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        (true, true) => a.cmp(b),
        (false, false) => std::cmp::Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::cmp_versions;
    use std::cmp::Ordering::*;
    #[test]
    fn versions() {
        assert_eq!(cmp_versions("0.10.0", "0.9.9"), Greater);
        assert_eq!(cmp_versions("v1.0", "1.0.0"), Equal);
        assert_eq!(cmp_versions("1.0.0-beta.1", "1.0.0"), Less);
        assert_eq!(cmp_versions("0.37.1", "0.37.10"), Less);
    }
}
