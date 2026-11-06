//! Accès réseau : Releases GitHub et téléchargements.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::io::{Read, Write};
use std::path::Path;

const UA: &str = concat!("FCDI-Apps/", env!("CARGO_PKG_VERSION"));

fn agent(https_only: bool) -> ureq::Agent {
    ureq::Agent::config_builder()
        // Jamais de repli en http (ni lors d'une redirection) : sinon un intermédiaire pourrait altérer les fichiers.
        .https_only(https_only)
        .timeout_connect(Some(std::time::Duration::from_secs(15)))
        .http_status_as_error(false)
        // Certificats du système (comme le navigateur), utile derrière un proxy d'entreprise.
        .tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
        .build()
        .into()
}

fn request(url: &str) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
    // Le http n'est toléré que vers la machine locale (tests).
    let https_only = !url.starts_with("http://127.0.0.1");
    let mut req = agent(https_only).get(url).header("User-Agent", UA);
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

/// Cache disque des Releases : le corps JSON et son ETag. Une requête « rien de nouveau » (HTTP 304)
/// ne compte pas dans la limite de 60 requêtes par heure de l'API GitHub.
fn cache_files(repo: &str) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let dir = dirs::cache_dir()?.join("fcdi-apps").join("releases");
    let name = repo.replace('/', "__");
    Some((dir.join(format!("{name}.json")), dir.join(format!("{name}.etag"))))
}

fn parse_releases(text: &str) -> Result<Vec<Release>> {
    let mut list: Vec<Release> = serde_json::from_str(text)?;
    list.retain(|r| !r.draft);
    Ok(list)
}

fn fetch_releases(url: &str, etag: Option<&str>) -> Result<Option<(String, Option<String>)>> {
    let mut req = request(url);
    if let Some(e) = etag {
        req = req.header("If-None-Match", e);
    }
    let mut resp = req.call().with_context(|| format!("connexion à {url}"))?;
    let status = resp.status().as_u16();
    if status == 304 {
        return Ok(None);
    }
    let new_etag = resp.headers().get("etag").and_then(|v| v.to_str().ok()).map(str::to_string);
    let body = resp.body_mut().read_to_string()?;
    if !(200..300).contains(&status) {
        if (status == 403 || status == 429) && body.contains("rate limit") {
            bail!("limite de l'API GitHub atteinte, réessayez dans une heure");
        }
        bail!("{url} : HTTP {status}");
    }
    Ok(Some((body, new_etag)))
}

/// Releases publiées d'un dépôt, de la plus récente à la plus ancienne.
/// En cas d'échec (limite atteinte, hors ligne), renvoie les dernières Releases connues si elles existent.
pub fn releases(repo: &str) -> Result<Vec<Release>> {
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=50");
    let files = cache_files(repo);
    let cached_body = files.as_ref().and_then(|(b, _)| std::fs::read_to_string(b).ok());
    let cached_etag = files.as_ref().and_then(|(_, e)| std::fs::read_to_string(e).ok());
    // Sans corps en cache, un 304 serait inutilisable : pas d'ETag dans ce cas.
    let etag = cached_body.as_ref().and(cached_etag.as_deref());
    match fetch_releases(&url, etag) {
        Ok(None) => parse_releases(cached_body.as_deref().unwrap_or("[]")),
        Ok(Some((body, new_etag))) => {
            let list = parse_releases(&body)?;
            if let Some((bf, ef)) = &files {
                let _ = std::fs::create_dir_all(bf.parent().unwrap());
                let _ = std::fs::write(bf, &body);
                match new_etag {
                    Some(e) => {
                        let _ = std::fs::write(ef, e);
                    }
                    None => {
                        let _ = std::fs::remove_file(ef);
                    }
                }
            }
            Ok(list)
        }
        Err(e) => match cached_body.as_deref().map(parse_releases) {
            Some(Ok(list)) => Ok(list),
            _ => Err(e),
        },
    }
}

/// Télécharge `url` vers `dest` (via un fichier `.part`), en appelant `progress(fait, total)`.
pub fn download(url: &str, dest: &Path, progress: impl FnMut(u64, u64)) -> Result<()> {
    download_checked(url, dest, progress, |_| Ok(())).map(|_| ())
}

/// Comme `download`, mais le fichier n'est mis à sa place définitive qu'après `check(empreinte SHA-256)` :
/// si la vérification échoue, rien n'est installé et le fichier partiel est supprimé.
pub fn download_checked(
    url: &str,
    dest: &Path,
    mut progress: impl FnMut(u64, u64),
    check: impl FnOnce(&str) -> Result<()>,
) -> Result<String> {
    use sha2::{Digest, Sha256};
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
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    let result = (|| -> Result<String> {
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
            hasher.update(&buf[..n]);
            done += n as u64;
            progress(done, total);
        }
        file.sync_all()?;
        if total > 0 && done != total {
            bail!("téléchargement incomplet ({done} / {total} octets)");
        }
        let hex: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
        check(&hex)?;
        Ok(hex)
    })();
    drop(file);
    match result {
        Ok(hex) => {
            std::fs::rename(&part, dest)?;
            Ok(hex)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(e)
        }
    }
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
    use super::{cmp_versions, fetch_releases};
    use std::cmp::Ordering::*;
    use std::io::{Read, Write};

    /// Petit serveur local : 200 + ETag, puis 304 si l'ETag est renvoyé, puis 403 « rate limit ».
    #[test]
    fn etag_et_limite() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for (i, stream) in listener.incoming().enumerate() {
                let mut stream = stream.unwrap();
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap();
                let req = String::from_utf8_lossy(&buf[..n]).to_lowercase();
                let reply = match i {
                    0 => "HTTP/1.1 200 OK\r\nETag: \"abc\"\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]".to_string(),
                    1 => {
                        assert!(req.contains("if-none-match: \"abc\""));
                        "HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n".to_string()
                    }
                    _ => {
                        let body = "API rate limit exceeded";
                        format!("HTTP/1.1 403 Forbidden\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
                    }
                };
                stream.write_all(reply.as_bytes()).unwrap();
            }
        });
        let url = format!("http://127.0.0.1:{port}/x");
        let (body, etag) = fetch_releases(&url, None).unwrap().unwrap();
        assert_eq!((body.as_str(), etag.as_deref()), ("[]", Some("\"abc\"")));
        assert!(fetch_releases(&url, Some("\"abc\"")).unwrap().is_none());
        let err = fetch_releases(&url, None).unwrap_err().to_string();
        assert!(err.contains("limite"), "{err}");
    }

    /// Un fichier refusé par la vérification n'est jamais mis en place ; un fichier accepté l'est, avec son SHA-256.
    #[test]
    fn telechargement_verifie() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf).unwrap();
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello").unwrap();
            }
        });
        let url = format!("http://127.0.0.1:{port}/f");
        let dir = std::env::temp_dir().join(format!("fcdi-test-{port}"));
        let _ = std::fs::remove_dir_all(&dir);
        let dest = dir.join("app.AppImage");
        let refuse = super::download_checked(&url, &dest, |_, _| {}, |_| anyhow::bail!("refusé"));
        assert!(refuse.is_err());
        assert!(!dest.exists() && !dest.with_extension("part").exists());
        let sha = super::download_checked(&url, &dest, |_, _| {}, |_| Ok(())).unwrap();
        assert_eq!(sha, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn versions() {
        assert_eq!(cmp_versions("0.10.0", "0.9.9"), Greater);
        assert_eq!(cmp_versions("v1.0", "1.0.0"), Equal);
        assert_eq!(cmp_versions("1.0.0-beta.1", "1.0.0"), Less);
        assert_eq!(cmp_versions("0.37.1", "0.37.10"), Less);
    }
}
