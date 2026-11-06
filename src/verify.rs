//! Vérification des fichiers téléchargés : signature Ed25519 + empreinte SHA-256.
//!
//! Chaque AppImage publié dans une Release est accompagné d'un fichier `<nom>.sig` (signature en base64).
//! La signature porte sur un message qui lie l'empreinte du fichier à l'application, à la version
//! (étiquette de la Release) et au nom du fichier : on ne peut ni échanger un fichier contre un autre,
//! ni le rejouer sous une autre version. Les clés publiques acceptées sont dans `keys/signing.pub`.

use anyhow::{Result, bail};
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

const KEYS: &str = include_str!("../keys/signing.pub");

/// Applications dont les Releases ne sont pas encore signées. Cette liste est figée dans le lanceur :
/// seule une nouvelle version signée du lanceur peut la modifier (retirer un identifiant dès que l'application signe).
pub const LEGACY_UNSIGNED: &[&str] = &["luminux-ia"];

fn parse_keys(text: &str) -> Vec<VerifyingKey> {
    let b64 = base64::engine::general_purpose::STANDARD;
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let bytes: [u8; 32] = b64.decode(l).ok()?.try_into().ok()?;
            VerifyingKey::from_bytes(&bytes).ok()
        })
        .collect()
}

/// Message signé : version du format, application, version, nom de fichier, empreinte.
pub fn message(app_id: &str, tag: &str, asset: &str, sha256_hex: &str) -> Vec<u8> {
    format!("FCDI-Apps signature v1\n{app_id}\n{tag}\n{asset}\n{sha256_hex}\n").into_bytes()
}

fn verify_with(keys: &[VerifyingKey], msg: &[u8], sig_b64: &str) -> Result<()> {
    if keys.is_empty() {
        bail!("aucune clé de signature n'est configurée dans le lanceur : vérification impossible");
    }
    let b64 = base64::engine::general_purpose::STANDARD;
    let raw = b64.decode(sig_b64.trim()).map_err(|_| anyhow::anyhow!("signature illisible"))?;
    let sig = Signature::from_slice(&raw).map_err(|_| anyhow::anyhow!("signature de taille invalide"))?;
    if keys.iter().any(|k| k.verify(msg, &sig).is_ok()) {
        Ok(())
    } else {
        bail!("signature invalide : le fichier n'a pas été signé par FCDI (installation refusée)")
    }
}

/// Vérifie qu'un fichier téléchargé est authentique. `sig` est le contenu du fichier `.sig` de la Release, s'il existe.
pub fn check_download(app_id: &str, tag: &str, asset: &str, sha256_hex: &str, sig: Option<&str>) -> Result<()> {
    match sig {
        Some(sig) => verify_with(&parse_keys(KEYS), &message(app_id, tag, asset, sha256_hex), sig),
        None if LEGACY_UNSIGNED.contains(&app_id) => Ok(()),
        None => bail!("cette version n'a pas de signature (fichier .sig absent de la Release) : installation refusée"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }
    fn sign(msg: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(key().sign(msg).to_bytes())
    }

    #[test]
    fn signature_valide_et_alterations() {
        let keys = vec![key().verifying_key()];
        let msg = message("david-fit", "v0.1.36", "DavidFIT-x86_64.AppImage", "ab12");
        let sig = sign(&msg);
        assert!(verify_with(&keys, &msg, &sig).is_ok());
        // fichier différent, autre version, autre application, autre nom : refusés
        for bad in [
            message("david-fit", "v0.1.36", "DavidFIT-x86_64.AppImage", "ab13"),
            message("david-fit", "v0.1.35", "DavidFIT-x86_64.AppImage", "ab12"),
            message("luminux-ia", "v0.1.36", "DavidFIT-x86_64.AppImage", "ab12"),
            message("david-fit", "v0.1.36", "Autre-x86_64.AppImage", "ab12"),
        ] {
            assert!(verify_with(&keys, &bad, &sig).is_err());
        }
        // mauvaise clé, signature illisible, trop courte
        let other = vec![SigningKey::from_bytes(&[8u8; 32]).verifying_key()];
        assert!(verify_with(&other, &msg, &sig).is_err());
        assert!(verify_with(&keys, &msg, "pas du base64 !").is_err());
        assert!(verify_with(&keys, &msg, "AAAA").is_err());
        // sans clé configurée : on refuse
        assert!(verify_with(&[], &msg, &sig).is_err());
    }

    #[test]
    fn format_du_message() {
        // Doit rester identique à tools/sign.sh et au workflow desktop.yml de David FIT.
        assert_eq!(
            message("david-fit", "v0.1.37", "DavidFIT-x86_64.AppImage", "abc"),
            b"FCDI-Apps signature v1\ndavid-fit\nv0.1.37\nDavidFIT-x86_64.AppImage\nabc\n".to_vec()
        );
    }

    #[test]
    fn fichier_de_cles() {
        let b64 = base64::engine::general_purpose::STANDARD;
        let pk = b64.encode(key().verifying_key().to_bytes());
        let keys = parse_keys(&format!("# commentaire\n\n{pk}\nn'importe quoi\n"));
        assert_eq!(keys.len(), 1);
    }

    /// Garde-fou : une version du lanceur sans clé publique ne pourrait plus rien installer ni se mettre à jour.
    #[test]
    fn cle_publique_renseignee() {
        assert!(!parse_keys(KEYS).is_empty(), "keys/signing.pub ne contient aucune clé publique valide");
    }

    #[test]
    fn sans_signature() {
        assert!(check_download("luminux-ia", "v1", "a", "00", None).is_ok());
        assert!(check_download("david-fit", "v1", "a", "00", None).is_err());
    }

    /// Compatibilité avec la signature produite par `tools/sign.sh` (openssl) : exécuté seulement si
    /// FCDI_SIG_FIXTURE pointe vers un dossier contenant `pub.b64`, `msg` et `sig.b64`.
    #[test]
    fn compat_openssl() {
        let Ok(dir) = std::env::var("FCDI_SIG_FIXTURE") else { return };
        let read = |n: &str| std::fs::read(format!("{dir}/{n}")).unwrap();
        let keys = parse_keys(&String::from_utf8(read("pub.b64")).unwrap());
        assert_eq!(keys.len(), 1);
        verify_with(&keys, &read("msg"), &String::from_utf8(read("sig.b64")).unwrap()).unwrap();
    }
}
