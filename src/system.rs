//! Intégration au bureau Linux : lancement, raccourcis de menu, démarrage automatique.

use crate::catalog::AppEntry;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(path)?.permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(path, perm)?;
    Ok(())
}

/// Lance un programme détaché du lanceur.
pub fn launch(path: &Path) -> Result<()> {
    let mut child = Command::new(path)
        .current_dir(dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("lancement de {}", path.display()))?;
    // Récupère le processus à sa fin pour ne pas laisser de zombie.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Ouvre une adresse ou un dossier avec l'application par défaut.
pub fn open_url(target: &str) {
    let _ = Command::new("xdg-open")
        .arg(target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|mut c| std::thread::spawn(move || c.wait()));
}

pub fn notify(title: &str, body: &str) {
    let _ = Command::new("notify-send")
        .args(["-a", "FCDI Apps", title, body])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|mut c| std::thread::spawn(move || c.wait()));
}

pub fn data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("fcdi-apps")
}

pub fn icon_path(id: &str) -> PathBuf {
    data_dir().join("icons").join(format!("{id}.png"))
}

fn desktop_path(id: &str) -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("applications").join(format!("fcdi-{id}.desktop")))
}

fn quote(p: &Path) -> String {
    format!("\"{}\"", p.display().to_string().replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn write_desktop_entry(app: &AppEntry, exe: &Path) -> Result<()> {
    let Some(file) = desktop_path(&app.id) else { return Ok(()) };
    std::fs::create_dir_all(file.parent().unwrap())?;
    let icon = icon_path(&app.id);
    let text = format!(
        "[Desktop Entry]\nType=Application\nName={}\nComment={}\nExec={}\nIcon={}\nTerminal=false\nCategories=Utility;\nX-FCDI-Apps=true\n",
        app.full_name(),
        app.description,
        quote(exe),
        if icon.exists() { icon.display().to_string() } else { "application-x-executable".into() },
    );
    std::fs::write(file, text)?;
    Ok(())
}

pub fn remove_desktop_entry(id: &str) {
    if let Some(p) = desktop_path(id) {
        let _ = std::fs::remove_file(p);
    }
}

/// Chemin du lanceur lui-même (l'AppImage si lancé depuis une AppImage).
pub fn self_path() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE").map(PathBuf::from).or_else(|| std::env::current_exe().ok())
}

pub fn set_autostart(on: bool) -> Result<()> {
    let Some(dir) = dirs::config_dir().map(|d| d.join("autostart")) else { return Ok(()) };
    let file = dir.join("fcdi-apps.desktop");
    if !on {
        let _ = std::fs::remove_file(file);
        return Ok(());
    }
    let exe = self_path().context("chemin du lanceur introuvable")?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        file,
        format!(
            "[Desktop Entry]\nType=Application\nName=FCDI Apps\nExec={} --minimized\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
            quote(&exe)
        ),
    )?;
    Ok(())
}
