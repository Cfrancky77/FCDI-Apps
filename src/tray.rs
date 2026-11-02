//! Icône dans la zone de notification (StatusNotifierItem, via D-Bus).
//! Sans hôte de zone de notification sur le bureau, `start` renvoie `None` et la fenêtre se ferme normalement.

use ksni::blocking::{Handle, TrayMethods};
use std::sync::mpsc::Sender;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCmd {
    Show,
    Quit,
}

pub struct FcdiTray {
    tx: Sender<TrayCmd>,
    ctx: eframe::egui::Context,
    label_open: String,
    label_quit: String,
}

impl FcdiTray {
    fn send(&self, cmd: TrayCmd) {
        let _ = self.tx.send(cmd);
        // Réveille la boucle même si la fenêtre est cachée.
        self.ctx.request_repaint();
    }
}

impl ksni::Tray for FcdiTray {
    fn id(&self) -> String {
        "fcdi-apps".into()
    }

    fn title(&self) -> String {
        "FCDI Apps".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        pixmaps()
    }

    // Clic gauche sur l'icône.
    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayCmd::Show);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        vec![
            StandardItem {
                label: self.label_open.clone(),
                activate: Box::new(|t: &mut Self| t.send(TrayCmd::Show)),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: self.label_quit.clone(),
                activate: Box::new(|t: &mut Self| t.send(TrayCmd::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Icône de l'application en ARGB32 (ordre réseau), à plusieurs tailles.
fn pixmaps() -> Vec<ksni::Icon> {
    let Ok(img) = image::load_from_memory(include_bytes!("../packaging/fcdi-apps.png")) else { return Vec::new() };
    [24u32, 32, 48, 64]
        .into_iter()
        .map(|s| {
            let small = img.resize_exact(s, s, image::imageops::FilterType::Lanczos3).to_rgba8();
            let mut data = Vec::with_capacity((s * s * 4) as usize);
            for p in small.pixels() {
                data.extend_from_slice(&[p[3], p[0], p[1], p[2]]);
            }
            ksni::Icon { width: s as i32, height: s as i32, data }
        })
        .collect()
}

pub fn start(
    tx: Sender<TrayCmd>,
    ctx: eframe::egui::Context,
    label_open: &str,
    label_quit: &str,
) -> Option<Handle<FcdiTray>> {
    FcdiTray { tx, ctx, label_open: label_open.into(), label_quit: label_quit.into() }.spawn().ok()
}
