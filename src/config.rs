//! Réglages et état des installations, dans ~/.config/fcdi-apps/config.json.

use crate::i18n::Lang;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Debug)]
pub enum Theme {
    #[default]
    System,
    Dark,
    Light,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Installed {
    pub version: String,
    pub tag: String,
    pub path: PathBuf,
    /// Version choisie à la main : pas de mise à jour automatique.
    #[serde(default)]
    pub pinned: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub lang: Lang,
    pub theme: Theme,
    pub install_dir: PathBuf,
    pub auto_check: bool,
    pub auto_install: bool,
    pub notify: bool,
    pub prereleases: bool,
    pub autostart: bool,
    /// Fermer la fenêtre garde le lanceur actif dans la zone de notification (si le bureau en a une).
    pub keep_in_tray: bool,
    pub desktop_entries: bool,
    pub show_hidden: bool,
    pub grid: bool,
    pub hidden: Vec<String>,
    pub installed: BTreeMap<String, Installed>,
}

pub fn default_install_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("Applications")
}

impl Default for Config {
    fn default() -> Self {
        let lang = match std::env::var("LANG") {
            Ok(l) if !l.starts_with("fr") && !l.is_empty() && l != "C" && l != "C.UTF-8" => Lang::En,
            _ => Lang::Fr,
        };
        Self {
            lang,
            theme: Theme::System,
            install_dir: default_install_dir(),
            auto_check: true,
            auto_install: false,
            notify: true,
            prereleases: false,
            autostart: false,
            keep_in_tray: true,
            desktop_entries: true,
            show_hidden: false,
            grid: false,
            hidden: Vec::new(),
            installed: BTreeMap::new(),
        }
    }
}

fn path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("fcdi-apps").join("config.json")
}

impl Config {
    pub fn load() -> Self {
        let mut cfg: Config = std::fs::read_to_string(path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        // Oublie une installation dont le fichier a été supprimé à la main.
        cfg.installed.retain(|_, i| i.path.exists());
        cfg
    }

    pub fn save(&self) {
        let p = path();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let tmp = p.with_extension("tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(tmp, p);
            }
        }
    }
}
