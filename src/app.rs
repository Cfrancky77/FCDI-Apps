//! Interface du lanceur.

use crate::catalog::{self, AppEntry, Catalog};
use crate::config::{Config, Installed, Theme};
use crate::i18n::{Lang, Tr, tr};
use crate::net::{self, Release};
use crate::system;
use crate::tray::{self, TrayCmd};
use crate::verify;
use eframe::egui::{
    self, Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, Sense, Stroke, TextureHandle, Vec2,
};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const SELF_REPO: &str = "Cfrancky77/FCDI-Apps";
const SELF_ASSET: &str = "FCDI_Apps-x86_64.AppImage";
const SELF_ID: &str = "fcdi-apps";
const CHECK_EVERY: Duration = Duration::from_secs(6 * 3600);

enum Msg {
    Catalog(Catalog),
    Releases(String, Result<Vec<Release>, String>),
    Progress(String, u64, u64),
    InstallDone(String, Result<Installed, String>),
    Icon(String, Vec<u8>),
    SelfReleases(Result<Vec<Release>, String>),
    SelfUpdated(Result<(), String>),
}

#[derive(Default)]
struct AppState {
    releases: Option<Result<Vec<Release>, String>>,
    progress: Option<(u64, u64)>,
    icon: Option<TextureHandle>,
}

enum Dialog {
    Versions(String),
    Notes(String),
    Uninstall(String),
    Error(String),
}

pub struct Launcher {
    cfg: Config,
    catalog: Catalog,
    states: HashMap<String, AppState>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    ctx: egui::Context,
    search: String,
    dialog: Option<Dialog>,
    settings_open: bool,
    pending: usize,
    last_check: Option<Instant>,
    found_updates: Vec<String>,
    self_releases: Option<Result<Vec<Release>, String>>,
    self_progress: Option<(u64, u64)>,
    self_updated: bool,
    start_minimized: bool,
    banner: Option<TextureHandle>,
    tray: Option<ksni::blocking::Handle<tray::FcdiTray>>,
    tray_rx: Receiver<TrayCmd>,
    /// Vrai quand l'arrêt est voulu (menu Quitter, bouton Quitter, relance) : la fermeture ne doit pas se cacher.
    quitting: bool,
}

fn hex(c: &str) -> Option<Color32> {
    let c = c.trim_start_matches('#');
    if c.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(c, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// « 2026-11-01T10:00:00Z » → « 1 nov. 2026 ».
fn short_date(iso: &str, lang: Lang) -> String {
    let d: Vec<&str> = iso.get(..10).unwrap_or("").split('-').collect();
    if d.len() != 3 {
        return String::new();
    }
    let m: usize = d[1].parse().unwrap_or(1);
    let fr = ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."];
    let en = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let day: u32 = d[2].parse().unwrap_or(1);
    match lang {
        Lang::Fr => format!("{day} {} {}", fr[(m - 1).min(11)], d[0]),
        Lang::En => format!("{} {day}, {}", en[(m - 1).min(11)], d[0]),
    }
}

fn human_size(b: u64) -> String {
    const KO: f64 = 1024.0;
    let b = b as f64;
    if b >= KO * KO * KO {
        format!("{:.1} Go", b / (KO * KO * KO))
    } else if b >= KO * KO {
        format!("{:.0} Mo", b / (KO * KO))
    } else {
        format!("{:.0} Ko", b / KO)
    }
}

fn apply_theme(ctx: &egui::Context, theme: Theme) {
    ctx.set_theme(match theme {
        Theme::System => egui::ThemePreference::System,
        Theme::Dark => egui::ThemePreference::Dark,
        Theme::Light => egui::ThemePreference::Light,
    });
}

fn setup_style(ctx: &egui::Context) {
    let mut dark = egui::Visuals::dark();
    dark.panel_fill = Color32::from_rgb(0x12, 0x14, 0x17);
    dark.window_fill = Color32::from_rgb(0x17, 0x19, 0x1d);
    dark.extreme_bg_color = Color32::from_rgb(0x0c, 0x0d, 0x10);
    dark.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x2a, 0x2d, 0x33));
    dark.selection.bg_fill = Color32::from_rgb(0x2f, 0x6f, 0xe0);
    ctx.set_visuals_of(egui::Theme::Dark, dark);

    let mut light = egui::Visuals::light();
    light.panel_fill = Color32::from_rgb(0xf5, 0xf6, 0xf8);
    light.window_fill = Color32::WHITE;
    light.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0xd9, 0xdc, 0xe1));
    ctx.set_visuals_of(egui::Theme::Light, light);

    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(theme, |s| {
            s.spacing.item_spacing = Vec2::new(8.0, 6.0);
            s.spacing.button_padding = Vec2::new(10.0, 5.0);
        });
    }
}

impl Launcher {
    pub fn new(cc: &eframe::CreationContext<'_>, minimized: bool) -> Self {
        let cfg = Config::load();
        setup_style(&cc.egui_ctx);
        apply_theme(&cc.egui_ctx, cfg.theme);
        let catalog = catalog::cached().unwrap_or_else(catalog::embedded);
        let (tx, rx) = channel();
        let mut me = Self {
            cfg,
            catalog,
            states: HashMap::new(),
            tx,
            rx,
            ctx: cc.egui_ctx.clone(),
            search: String::new(),
            dialog: None,
            settings_open: false,
            pending: 0,
            last_check: None,
            found_updates: Vec::new(),
            self_releases: None,
            self_progress: None,
            self_updated: false,
            start_minimized: minimized,
            banner: load_banner(&cc.egui_ctx),
            tray: None,
            tray_rx: channel().1,
            quitting: false,
        };
        let (ttx, trx) = channel();
        me.tray = tray::start(ttx, cc.egui_ctx.clone(), me.t().tray_open, me.t().tray_quit);
        me.tray_rx = trx;
        me.load_icons();
        me.refresh();
        me
    }

    fn t(&self) -> &'static Tr {
        tr(self.cfg.lang)
    }

    fn spawn(&self, f: impl FnOnce(&Sender<Msg>) + Send + 'static) {
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            f(&tx);
            ctx.request_repaint();
        });
    }

    /// Relit le catalogue puis les Releases de chaque application et du lanceur.
    fn refresh(&mut self) {
        if self.pending > 0 {
            return;
        }
        self.pending = 1;
        self.found_updates.clear();
        self.spawn(|tx| {
            let cat = catalog::fetch().or_else(|_| catalog::cached().ok_or(())).unwrap_or_else(|_| catalog::embedded());
            let _ = tx.send(Msg::Catalog(cat));
        });
        self.spawn(|tx| {
            let _ = tx.send(Msg::SelfReleases(net::releases(SELF_REPO).map_err(|e| e.to_string())));
        });
    }

    fn load_icons(&mut self) {
        for app in self.catalog.apps.clone() {
            let st = self.states.entry(app.id.clone()).or_default();
            if st.icon.is_some() {
                continue;
            }
            let path = system::icon_path(&app.id);
            if let Ok(bytes) = std::fs::read(&path) {
                st.icon = load_texture(&self.ctx, &app.id, &bytes);
            } else if !app.icon.is_empty() {
                let (id, url) = (app.id.clone(), app.icon.clone());
                self.spawn(move |tx| {
                    if net::download(&url, &path, |_, _| {}).is_ok() {
                        if let Ok(bytes) = std::fs::read(&path) {
                            let _ = tx.send(Msg::Icon(id, bytes));
                        }
                    }
                });
            }
        }
    }

    /// Dernière Release utilisable (avec le bon fichier, préversions selon le réglage).
    fn latest<'a>(&self, releases: &'a [Release], asset: &str, id: &str) -> Option<&'a Release> {
        releases
            .iter()
            .filter(|r| self.cfg.prereleases || !r.prerelease)
            // Une version sans signature n'est jamais proposée (sauf applications de la liste « non signées »).
            .find(|r| r.asset_matching(asset).is_some_and(|a| is_signed(r, a, id)))
    }

    fn app(&self, id: &str) -> Option<AppEntry> {
        self.catalog.apps.iter().find(|a| a.id == id).cloned()
    }

    fn has_update(&self, app: &AppEntry) -> Option<Release> {
        let inst = self.cfg.installed.get(&app.id)?;
        let rels = self.states.get(&app.id)?.releases.as_ref()?.as_ref().ok()?;
        let latest = self.latest(rels, &app.asset, &app.id)?;
        (net::cmp_versions(&latest.version(), &inst.version) == std::cmp::Ordering::Greater).then(|| latest.clone())
    }

    fn install(&mut self, app: &AppEntry, rel: &Release, pinned: bool) {
        let Some(asset) = rel.asset_matching(&app.asset).cloned() else { return };
        if !safe_file_name(&asset.name) {
            self.dialog = Some(Dialog::Error(format!("nom de fichier refusé : {}", asset.name)));
            return;
        }
        let st = self.states.entry(app.id.clone()).or_default();
        if st.progress.is_some() {
            return;
        }
        st.progress = Some((0, asset.size));
        // Une mise à jour garde l'emplacement actuel ; une nouvelle installation va dans le dossier choisi.
        let dest = match self.cfg.installed.get(&app.id) {
            Some(i) => i.path.clone(),
            None => self.cfg.install_dir.join(&asset.name),
        };
        let (id, version, tag) = (app.id.clone(), rel.version(), rel.tag_name.clone());
        let rel = rel.clone();
        self.spawn(move |tx| {
            let res = (|| -> anyhow::Result<Installed> {
                let mut last = Instant::now();
                let sig = fetch_sig(&rel, &asset.name);
                // Le fichier n'est mis en place qu'après vérification de sa signature.
                net::download_checked(
                    &asset.browser_download_url,
                    &dest,
                    |d, t| {
                        if last.elapsed() > Duration::from_millis(150) {
                            last = Instant::now();
                            let _ = tx.send(Msg::Progress(id.clone(), d, t));
                        }
                    },
                    |sha| verify::check_download(&id, &rel.tag_name, &asset.name, sha, sig.as_deref()),
                )?;
                system::make_executable(&dest)?;
                Ok(Installed { version, tag, path: dest, pinned })
            })();
            let _ = tx.send(Msg::InstallDone(id, res.map_err(|e| format!("{e:#}"))));
        });
    }

    fn uninstall(&mut self, id: &str) {
        if let Some(inst) = self.cfg.installed.remove(id) {
            let _ = std::fs::remove_file(&inst.path);
            system::remove_desktop_entry(id);
            self.cfg.save();
        }
    }

    fn launch(&mut self, id: &str) {
        if let Some(inst) = self.cfg.installed.get(id) {
            if let Err(e) = system::launch(&inst.path) {
                self.dialog = Some(Dialog::Error(format!("{e:#}")));
            }
        }
    }

    fn update_self(&mut self) {
        let Some(Ok(rels)) = &self.self_releases else { return };
        let Some(rel) = self.latest(rels, SELF_ASSET, SELF_ID).cloned() else { return };
        let Some(asset) = rel.asset_matching(SELF_ASSET).cloned() else { return };
        let Some(dest) = std::env::var_os("APPIMAGE").map(std::path::PathBuf::from) else {
            system::open_url(&rel.html_url);
            return;
        };
        self.self_progress = Some((0, asset.size));
        self.spawn(move |tx| {
            let sig = fetch_sig(&rel, &asset.name);
            let res = net::download_checked(&asset.browser_download_url, &dest, |_, _| {}, |sha| {
                verify::check_download(SELF_ID, &rel.tag_name, &asset.name, sha, sig.as_deref())
            })
            .and_then(|_| system::make_executable(&dest));
            let _ = tx.send(Msg::SelfUpdated(res.map_err(|e| format!("{e:#}"))));
        });
    }

    fn handle_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Catalog(cat) => {
                    self.catalog = cat;
                    self.load_icons();
                    self.pending = self.catalog.apps.len();
                    for app in self.catalog.apps.clone() {
                        self.spawn(move |tx| {
                            let r = net::releases(&app.repo).map_err(|e| format!("{e:#}"));
                            let _ = tx.send(Msg::Releases(app.id, r));
                        });
                    }
                    if self.pending == 0 {
                        self.last_check = Some(Instant::now());
                    }
                }
                Msg::Releases(id, r) => {
                    self.states.entry(id.clone()).or_default().releases = Some(r);
                    if let Some(app) = self.app(&id) {
                        let pinned = self.cfg.installed.get(&id).is_some_and(|i| i.pinned);
                        if let Some(rel) = self.has_update(&app) {
                            if !pinned {
                                if self.cfg.auto_install {
                                    self.install(&app, &rel, false);
                                } else {
                                    self.found_updates.push(app.full_name());
                                }
                            }
                        }
                    }
                    self.pending = self.pending.saturating_sub(1);
                    if self.pending == 0 {
                        self.last_check = Some(Instant::now());
                        if self.cfg.notify && !self.found_updates.is_empty() {
                            let t = self.t();
                            system::notify("FCDI Apps", &t.notif_updates.replace("{}", &self.found_updates.join(", ")));
                        }
                    }
                }
                Msg::Progress(id, d, t) => {
                    if let Some(st) = self.states.get_mut(&id) {
                        st.progress = Some((d, t));
                    }
                }
                Msg::InstallDone(id, r) => {
                    if let Some(st) = self.states.get_mut(&id) {
                        st.progress = None;
                    }
                    match r {
                        Ok(inst) => {
                            if self.cfg.desktop_entries {
                                if let Some(app) = self.app(&id) {
                                    let _ = system::write_desktop_entry(&app, &inst.path);
                                }
                            }
                            self.cfg.installed.insert(id, inst);
                            self.cfg.save();
                        }
                        Err(e) => self.dialog = Some(Dialog::Error(e)),
                    }
                }
                Msg::Icon(id, bytes) => {
                    let tex = load_texture(&self.ctx, &id, &bytes);
                    self.states.entry(id).or_default().icon = tex;
                }
                Msg::SelfReleases(r) => {
                    self.self_releases = Some(r);
                    // Mise à jour automatique du lanceur (seulement s'il tourne en AppImage).
                    if self.cfg.auto_install
                        && !self.self_updated
                        && self.self_progress.is_none()
                        && std::env::var_os("APPIMAGE").is_some()
                        && self.self_update_available().is_some()
                    {
                        self.update_self();
                    }
                }
                Msg::SelfUpdated(r) => {
                    self.self_progress = None;
                    match r {
                        Ok(()) => self.self_updated = true,
                        Err(e) => self.dialog = Some(Dialog::Error(e)),
                    }
                }
            }
        }
    }

    /// Nouvelle version du lanceur, si elle est plus récente que celle qui tourne.
    fn self_update_available(&self) -> Option<Release> {
        self.self_latest().filter(|r| net::cmp_versions(&r.version(), VERSION) == std::cmp::Ordering::Greater)
    }

    fn self_latest(&self) -> Option<Release> {
        let rels = self.self_releases.as_ref()?.as_ref().ok()?;
        self.latest(rels, SELF_ASSET, SELF_ID).cloned()
    }
}

/// Contenu du fichier `<fichier>.sig` publié dans la Release, s'il existe.
fn fetch_sig(rel: &Release, asset_name: &str) -> Option<String> {
    let want = format!("{asset_name}.sig");
    let a = rel.assets.iter().find(|a| a.name == want)?;
    net::get_string(&a.browser_download_url).ok()
}

/// La Release contient-elle la signature du fichier (ou l'application est-elle dans la liste « non signées ») ?
fn is_signed(rel: &Release, asset: &net::Asset, id: &str) -> bool {
    verify::LEGACY_UNSIGNED.contains(&id) || rel.assets.iter().any(|s| s.name == format!("{}.sig", asset.name))
}

/// Nom de fichier simple : pas de séparateur de dossier ni de « .. ».
fn safe_file_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\', '\0']) && name != "." && name != ".."
}

/// Logo FCDI de l'en-tête (fichier fourni par Franck, à remplacer par une version de meilleure qualité).
fn load_banner(ctx: &egui::Context) -> Option<TextureHandle> {
    let img = image::load_from_memory(include_bytes!("../assets/banniere.png")).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture("banniere", color, egui::TextureOptions::LINEAR))
}

fn load_texture(ctx: &egui::Context, id: &str, bytes: &[u8]) -> Option<TextureHandle> {
    let img = image::load_from_memory(bytes).ok()?.resize(128, 128, image::imageops::FilterType::Lanczos3).to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture(format!("icon-{id}"), color, egui::TextureOptions::LINEAR))
}

// ---------------------------------------------------------------------------------------------
// Dessin

fn muted(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode { Color32::from_rgb(0x8b, 0x90, 0x99) } else { Color32::from_rgb(0x5f, 0x64, 0x6d) }
}

fn strong(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode { Color32::from_rgb(0xf2, 0xf3, 0xf5) } else { Color32::from_rgb(0x10, 0x12, 0x15) }
}

fn line(ui: &egui::Ui) -> Color32 {
    ui.visuals().widgets.noninteractive.bg_stroke.color
}

/// Petites capitales espacées (police proportionnelle : la police monospace n'a pas les accents).
fn mono(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).size(size).extra_letter_spacing(1.2)
}

#[derive(Clone, Copy)]
enum Icon {
    Gear,
    Refresh,
    List,
    Grid,
}

/// Bouton carré avec une icône dessinée (les polices par défaut n'ont pas ces symboles).
fn icon_button(ui: &mut egui::Ui, icon: Icon, selected: bool, framed: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
    let p = ui.painter();
    let fg = if selected { ui.visuals().panel_fill } else { strong(ui) };
    if selected {
        p.rect_filled(rect, CornerRadius::same(1), strong(ui));
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(1), ui.visuals().widgets.hovered.weak_bg_fill);
    }
    if framed && !selected {
        p.rect_stroke(rect, CornerRadius::same(1), Stroke::new(1.0, line(ui)), egui::StrokeKind::Inside);
    }
    let c = rect.center();
    let st = Stroke::new(1.6, fg);
    match icon {
        Icon::Gear => {
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let d = Vec2::new(a.cos(), a.sin());
                p.line_segment([c + d * 6.0, c + d * 9.5], Stroke::new(2.6, fg));
            }
            p.circle_stroke(c, 6.0, st);
            p.circle_stroke(c, 2.4, st);
        }
        Icon::Refresh => {
            let r = 7.5;
            let pts: Vec<egui::Pos2> = (0..=24)
                .map(|i| {
                    let a = -1.2 + i as f32 / 24.0 * 5.2;
                    c + Vec2::new(a.cos(), a.sin()) * r
                })
                .collect();
            let end = *pts.last().unwrap();
            p.add(egui::Shape::line(pts, st));
            let a: f32 = -1.2 + 5.2;
            let tangent = Vec2::new(-a.sin(), a.cos());
            let normal = Vec2::new(a.cos(), a.sin());
            p.line_segment([end, end - tangent * 4.0 + normal * 3.0], st);
            p.line_segment([end, end - tangent * 4.0 - normal * 3.0], st);
        }
        Icon::List => {
            for dy in [-5.0, 0.0, 5.0] {
                p.line_segment([c + Vec2::new(-7.0, dy), c + Vec2::new(7.0, dy)], st);
            }
        }
        Icon::Grid => {
            for (dx, dy) in [(-4.0, -4.0), (4.0, -4.0), (-4.0, 4.0), (4.0, 4.0)] {
                let r = egui::Rect::from_center_size(c + Vec2::new(dx, dy), Vec2::splat(6.0));
                p.rect_filled(r, CornerRadius::same(1), fg);
            }
        }
    }
    resp
}

fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    Frame::new()
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.7)))
        .inner_margin(Margin::symmetric(5, 2))
        .corner_radius(2)
        .show(ui, |ui| ui.label(mono(text, 9.5).color(color)));
}

/// Gros bouton à bordure claire, façon ArtCraft.
fn action_button(ui: &mut egui::Ui, text: &str, primary: bool, width: f32) -> egui::Response {
    let (fill, fg) = if primary {
        (strong(ui), ui.visuals().panel_fill)
    } else {
        (Color32::TRANSPARENT, strong(ui))
    };
    ui.add(
        egui::Button::new(mono(text, 11.0).strong().color(fg))
            .fill(fill)
            .stroke(Stroke::new(1.0, strong(ui)))
            .corner_radius(1)
            .min_size(Vec2::new(width, 32.0)),
    )
}

fn app_icon(ui: &mut egui::Ui, app: &AppEntry, tex: Option<&TextureHandle>, size: f32) {
    if let Some(t) = tex {
        ui.add(egui::Image::from_texture(t).fit_to_exact_size(Vec2::splat(size)).corner_radius(8));
    } else {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
        let c = hex(&app.color).unwrap_or(Color32::from_rgb(0x3b, 0x82, 0xf6));
        ui.painter().rect_filled(rect, CornerRadius::same(8), c);
        let letter = app.name.chars().next().unwrap_or('?').to_string();
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            letter,
            egui::FontId::proportional(size * 0.5),
            Color32::WHITE,
        );
    }
}

fn app_name(ui: &mut egui::Ui, app: &AppEntry, size: f32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label(RichText::new(&app.name).size(size).strong().color(strong(ui)));
        if !app.accent.is_empty() {
            let c = hex(&app.color).unwrap_or(Color32::from_rgb(0x3b, 0x82, 0xf6));
            ui.label(RichText::new(&app.accent).size(size).strong().color(c));
        }
    });
}

enum Action {
    Install(String, Release, bool),
    Launch(String),
    Dialog(Dialog),
    ShowFiles(String),
    Url(String),
    ToggleHide(String),
}

impl Launcher {
    /// Cache la fenêtre (X11) ou la réduit (Wayland, où une fenêtre ne peut pas se cacher soi-même).
    fn hide_window(&self, ctx: &egui::Context) {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var("XDG_SESSION_TYPE").as_deref() != Ok("x11") {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
    }

    fn show_window(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    fn quit(&mut self, ctx: &egui::Context) {
        self.quitting = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl eframe::App for Launcher {
    /// Tourne aussi quand la fenêtre est cachée : vérifications de mise à jour et commandes de l'icône.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.start_minimized {
            self.start_minimized = false;
            if self.tray.is_some() && self.cfg.keep_in_tray {
                self.hide_window(ctx);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
        }
        while let Ok(cmd) = self.tray_rx.try_recv() {
            match cmd {
                TrayCmd::Show => self.show_window(ctx),
                TrayCmd::Quit => self.quit(ctx),
            }
        }
        self.handle_messages();
        if self.cfg.auto_check && self.pending == 0 && self.last_check.is_some_and(|t| t.elapsed() > CHECK_EVERY) {
            self.refresh();
        }
        ctx.request_repaint_after(Duration::from_secs(30));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Fermer la fenêtre : rester dans la zone de notification si elle est disponible.
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting && self.cfg.keep_in_tray && self.tray.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide_window(&ctx);
        }

        let t = self.t();
        let mut actions: Vec<Action> = Vec::new();

        // Bandeau du haut
        egui::Panel::top("top")
            .frame(Frame::new().fill(ui.visuals().panel_fill).inner_margin(Margin::symmetric(16, 10)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(b) = &self.banner {
                        let h = 26.0;
                        ui.add(egui::Image::from_texture(b).fit_to_exact_size(Vec2::new(h * b.size()[0] as f32 / b.size()[1] as f32, h)));
                        ui.label(RichText::new("APPS").size(20.0).strong().color(strong(ui)));
                    } else {
                        ui.label(RichText::new("FCDI APPS").size(20.0).strong().color(strong(ui)));
                    }
                    ui.add_space(14.0);
                    ui.label(mono(t.subtitle, 10.5).color(muted(ui)));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icon_button(ui, Icon::Gear, false, false).on_hover_text(t.settings).clicked() {
                            self.settings_open = true;
                        }
                    });
                });
            });

        // Barre du bas
        egui::Panel::bottom("bottom")
            .frame(Frame::new().fill(ui.visuals().panel_fill).inner_margin(Margin::symmetric(16, 8)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let status = if self.pending > 0 {
                        t.checking.to_string()
                    } else if let Some(at) = self.last_check {
                        let min = at.elapsed().as_secs() / 60;
                        if min == 0 { t.checked_now.to_string() } else { t.checked_min.replace("{}", &min.to_string()) }
                    } else {
                        String::new()
                    };
                    ui.label(mono(format!("{status} · {}", t.via_github), 10.0).color(muted(ui)));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(mono(format!("v{VERSION}"), 10.0).color(muted(ui)));
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(Frame::new().fill(ui.visuals().panel_fill).inner_margin(Margin::symmetric(16, 8)))
            .show(ui, |ui| {
                // Barre d'outils
                ui.horizontal(|ui| {
                    let r = icon_button(ui, Icon::Refresh, false, true);
                    if r.on_hover_text(t.refresh).clicked() {
                        self.refresh();
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icon_button(ui, Icon::List, !self.cfg.grid, true).clicked() {
                            self.cfg.grid = false;
                            self.cfg.save();
                        }
                        if icon_button(ui, Icon::Grid, self.cfg.grid, true).clicked() {
                            self.cfg.grid = true;
                            self.cfg.save();
                        }
                        ui.add(egui::TextEdit::singleline(&mut self.search).hint_text(t.search).desired_width(240.0));
                    });
                });
                ui.add_space(8.0);

                // Bandeau « nouvelle version du lanceur »
                if self.self_updated || self.self_update_available().is_some() {
                    let mut do_update = false;
                    let mut do_relaunch = false;
                    Frame::new()
                        .fill(Color32::from_rgb(0x2f, 0x6f, 0xe0).gamma_multiply(0.18))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(0x2f, 0x6f, 0xe0)))
                        .inner_margin(Margin::symmetric(14, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                if self.self_updated {
                                    ui.label(RichText::new(t.self_done).color(strong(ui)));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if action_button(ui, t.relaunch, true, 140.0).clicked() {
                                            do_relaunch = true;
                                        }
                                    });
                                } else if let Some(rel) = self.self_update_available() {
                                    let v = format!("v{VERSION} -> v{}", rel.version());
                                    ui.label(RichText::new(t.self_new.replace("{}", &v)).color(strong(ui)));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if self.self_progress.is_some() {
                                            ui.label(mono(t.downloading, 10.0).color(muted(ui)));
                                            ui.spinner();
                                        } else if action_button(ui, t.update, true, 160.0).clicked() {
                                            do_update = true;
                                        }
                                    });
                                }
                            });
                        });
                    ui.add_space(8.0);
                    if do_update {
                        self.update_self();
                    }
                    if do_relaunch {
                        if let Some(exe) = system::self_path() {
                            if system::launch(&exe).is_ok() {
                                self.quit(ui.ctx());
                            }
                        }
                    }
                }

                let q = self.search.to_lowercase();
                let apps: Vec<AppEntry> = self
                    .catalog
                    .apps
                    .iter()
                    .filter(|a| self.cfg.show_hidden || !self.cfg.hidden.contains(&a.id))
                    .filter(|a| {
                        q.is_empty()
                            || a.full_name().to_lowercase().contains(&q)
                            || a.description.to_lowercase().contains(&q)
                            || a.description_en.to_lowercase().contains(&q)
                    })
                    .cloned()
                    .collect();

                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if apps.is_empty() {
                        ui.add_space(40.0);
                        ui.vertical_centered(|ui| ui.label(RichText::new(t.no_app).color(muted(ui))));
                    }
                    if self.cfg.grid {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::splat(12.0);
                            for app in &apps {
                                self.card(ui, app, &mut actions);
                            }
                        });
                    } else {
                        for app in &apps {
                            self.row(ui, app, &mut actions);
                        }
                    }
                });
            });

        for a in actions {
            match a {
                Action::Install(id, rel, pinned) => {
                    if let Some(app) = self.app(&id) {
                        self.install(&app, &rel, pinned);
                    }
                }
                Action::Launch(id) => self.launch(&id),
                Action::Dialog(d) => self.dialog = Some(d),
                Action::ShowFiles(id) => {
                    if let Some(dir) = self.cfg.installed.get(&id).and_then(|i| i.path.parent().map(|p| p.to_path_buf())) {
                        system::open_url(&dir.display().to_string());
                    }
                }
                Action::Url(u) => system::open_url(&u),
                Action::ToggleHide(id) => {
                    if let Some(pos) = self.cfg.hidden.iter().position(|h| *h == id) {
                        self.cfg.hidden.remove(pos);
                    } else {
                        self.cfg.hidden.push(id);
                    }
                    self.cfg.save();
                }
            }
        }

        self.dialogs(ui);
        if self.settings_open {
            self.settings(ui);
        }
    }
}

impl Launcher {
    /// Bouton principal + menu « … » communs aux vues liste et grille.
    fn main_button(&self, ui: &mut egui::Ui, app: &AppEntry, width: f32, actions: &mut Vec<Action>) {
        let t = self.t();
        let st = self.states.get(&app.id);
        let installed = self.cfg.installed.get(&app.id);
        let latest = st
            .and_then(|s| s.releases.as_ref())
            .and_then(|r| r.as_ref().ok())
            .and_then(|r| self.latest(r, &app.asset, &app.id))
            .cloned();
        if let Some((d, total)) = st.and_then(|s| s.progress) {
            let frac = if total > 0 { d as f32 / total as f32 } else { 0.0 };
            ui.add_sized(
                [width, 32.0],
                egui::ProgressBar::new(frac).text(mono(format!("{} {}%", t.downloading, (frac * 100.0) as u32), 10.0)),
            );
            return;
        }
        match (installed, &latest) {
            (Some(_), Some(rel)) if self.has_update(app).is_some() => {
                if action_button(ui, t.update, true, width).clicked() {
                    actions.push(Action::Install(app.id.clone(), rel.clone(), false));
                }
            }
            (Some(_), _) => {
                if action_button(ui, t.open, false, width).clicked() {
                    actions.push(Action::Launch(app.id.clone()));
                }
            }
            (None, Some(rel)) => {
                if action_button(ui, t.install, true, width).clicked() {
                    actions.push(Action::Install(app.id.clone(), rel.clone(), false));
                }
            }
            (None, None) => {
                ui.add_enabled_ui(false, |ui| action_button(ui, t.unavailable, false, width));
            }
        }
    }

    fn menu(&self, ui: &mut egui::Ui, app: &AppEntry, actions: &mut Vec<Action>) {
        let t = self.t();
        let installed = self.cfg.installed.get(&app.id);
        let latest = self
            .states
            .get(&app.id)
            .and_then(|s| s.releases.as_ref())
            .and_then(|r| r.as_ref().ok())
            .and_then(|r| self.latest(r, &app.asset, &app.id))
            .cloned();
        let btn = egui::Button::new(RichText::new("…").size(14.0)).min_size(Vec2::new(34.0, 32.0)).corner_radius(1);
        egui::containers::menu::MenuButton::from_button(btn).ui(ui, |ui| {
            ui.set_min_width(220.0);
            if ui.button(t.m_choose_version).clicked() {
                actions.push(Action::Dialog(Dialog::Versions(app.id.clone())));
            }
            if ui.button(t.m_release_notes).clicked() {
                actions.push(Action::Dialog(Dialog::Notes(app.id.clone())));
            }
            if installed.is_some() {
                if ui.button(t.m_open).clicked() {
                    actions.push(Action::Launch(app.id.clone()));
                }
                if ui.button(t.m_show_files).clicked() {
                    actions.push(Action::ShowFiles(app.id.clone()));
                }
                let current = installed.and_then(|i| {
                    self.states
                        .get(&app.id)?
                        .releases
                        .as_ref()?
                        .as_ref()
                        .ok()?
                        .iter()
                        .find(|r| r.tag_name == i.tag)
                        .cloned()
                });
                if let Some(rel) = current.or(latest) {
                    if ui.button(t.m_reinstall).clicked() {
                        let pinned = installed.is_some_and(|i| i.pinned);
                        actions.push(Action::Install(app.id.clone(), rel, pinned));
                    }
                }
                ui.separator();
                if ui.button(RichText::new(t.m_uninstall).color(Color32::from_rgb(0xef, 0x44, 0x44))).clicked() {
                    actions.push(Action::Dialog(Dialog::Uninstall(app.id.clone())));
                }
            }
            ui.separator();
            if !app.website.is_empty() && ui.button(t.m_website).clicked() {
                actions.push(Action::Url(app.website.clone()));
            }
            if ui.button(t.m_repo).clicked() {
                actions.push(Action::Url(app.repo_url()));
            }
            if ui.button(t.m_all_versions).clicked() {
                actions.push(Action::Url(format!("{}/releases", app.repo_url())));
            }
            ui.separator();
            let hidden = self.cfg.hidden.contains(&app.id);
            if ui.button(if hidden { t.m_unhide } else { t.m_hide }).clicked() {
                actions.push(Action::ToggleHide(app.id.clone()));
            }
        });
    }

    fn status(&self, app: &AppEntry) -> (&'static str, Color32, String, String) {
        let t = self.t();
        let st = self.states.get(&app.id);
        let latest = st
            .and_then(|s| s.releases.as_ref())
            .and_then(|r| r.as_ref().ok())
            .and_then(|r| self.latest(r, &app.asset, &app.id));
        let date = latest.and_then(|r| r.published_at.as_deref()).map(|d| short_date(d, self.cfg.lang)).unwrap_or_default();
        match self.cfg.installed.get(&app.id) {
            Some(i) if self.has_update(app).is_some() => (
                t.update_badge,
                Color32::from_rgb(0xf5, 0x9e, 0x0b),
                format!("{} -> {}", i.version, latest.map(|r| r.version()).unwrap_or_default()),
                date,
            ),
            Some(i) => (t.installed, Color32::from_rgb(0x9c, 0xa3, 0xaf), i.version.clone(), date),
            None => (
                t.not_installed,
                Color32::from_rgb(0x6b, 0x72, 0x80),
                latest.map(|r| r.version()).unwrap_or_else(|| "—".into()),
                date,
            ),
        }
    }

    /// Poids du fichier de la dernière version proposée (vide si inconnu).
    fn size_label(&self, app: &AppEntry) -> String {
        self.states
            .get(&app.id)
            .and_then(|s| s.releases.as_ref())
            .and_then(|r| r.as_ref().ok())
            .and_then(|r| self.latest(r, &app.asset, &app.id))
            .and_then(|r| r.asset_matching(&app.asset))
            .filter(|a| a.size > 0)
            .map(|a| human_size(a.size))
            .unwrap_or_default()
    }

    fn row(&self, ui: &mut egui::Ui, app: &AppEntry, actions: &mut Vec<Action>) {
        let desc = if self.cfg.lang == Lang::En && !app.description_en.is_empty() { &app.description_en } else { &app.description };
        let (badge_text, badge_color, version, date) = self.status(app);
        let err = self
            .states
            .get(&app.id)
            .and_then(|s| s.releases.as_ref())
            .and_then(|r| r.as_ref().err())
            .cloned();
        let size = self.size_label(app);
        Frame::new()
            .inner_margin(Margin::symmetric(14, 12))
            .stroke(Stroke::new(1.0, line(ui)))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                // Colonnes de droite à largeur fixe : toutes les lignes sont alignées,
                // quelle que soit la longueur de la description.
                const RIGHT: f32 = 32.0 + 160.0 + 120.0 + 110.0 + 80.0 + 130.0 + 6.0 * 8.0;
                let left_w = (ui.available_width() - RIGHT).max(160.0);
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(Vec2::new(left_w, 44.0), Layout::left_to_right(Align::Center), |ui| {
                        ui.set_width(left_w);
                        app_icon(ui, app, self.states.get(&app.id).and_then(|s| s.icon.as_ref()), 40.0);
                        ui.vertical(|ui| {
                            app_name(ui, app, 16.0);
                            ui.add(egui::Label::new(RichText::new(desc).size(12.0).color(muted(ui))).truncate());
                            if let Some(e) = &err {
                                ui.label(RichText::new(e).size(11.0).color(Color32::from_rgb(0xef, 0x44, 0x44)));
                            }
                        });
                    });
                    // Nom | Version | Taille | Date | Statut | actions
                    ui.add_sized([130.0, 20.0], egui::Label::new(mono(&version, 13.0).color(strong(ui))));
                    ui.add_sized([80.0, 20.0], egui::Label::new(mono(&size, 12.0).color(muted(ui))));
                    ui.add_sized([110.0, 20.0], egui::Label::new(RichText::new(&date).color(muted(ui))));
                    ui.allocate_ui_with_layout(Vec2::new(120.0, 24.0), Layout::left_to_right(Align::Center), |ui| {
                        badge(ui, badge_text, badge_color)
                    });
                    self.main_button(ui, app, 160.0, actions);
                    self.menu(ui, app, actions);
                });
            });
    }

    fn card(&self, ui: &mut egui::Ui, app: &AppEntry, actions: &mut Vec<Action>) {
        let desc = if self.cfg.lang == Lang::En && !app.description_en.is_empty() { &app.description_en } else { &app.description };
        let (badge_text, badge_color, version, _) = self.status(app);
        Frame::new()
            .inner_margin(Margin::same(14))
            .stroke(Stroke::new(1.0, line(ui)))
            .show(ui, |ui| {
                ui.set_width(230.0);
                ui.set_height(210.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        app_icon(ui, app, self.states.get(&app.id).and_then(|s| s.icon.as_ref()), 48.0);
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| badge(ui, badge_text, badge_color));
                    });
                    app_name(ui, app, 16.0);
                    ui.label(RichText::new(desc).size(12.0).color(muted(ui)));
                    let size = self.size_label(app);
                    let line_text = if size.is_empty() { version.clone() } else { format!("{version}  ·  {size}") };
                    ui.label(mono(line_text, 12.0).color(strong(ui)));
                    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                        ui.horizontal(|ui| {
                            self.main_button(ui, app, 180.0, actions);
                            self.menu(ui, app, actions);
                        });
                    });
                });
            });
    }

    fn dialogs(&mut self, ui: &mut egui::Ui) {
        let Some(dialog) = &self.dialog else { return };
        let t = self.t();
        let mut close = false;
        let mut todo: Option<Action> = None;
        let mut do_uninstall: Option<String> = None;
        let modal = egui::Modal::new(egui::Id::unique("dialog")).show(ui.ctx(), |ui| {
            ui.set_width(520.0);
            match dialog {
                Dialog::Error(e) => {
                    ui.heading(t.error);
                    ui.label(e);
                }
                Dialog::Uninstall(id) => {
                    let name = self.app(id).map(|a| a.full_name()).unwrap_or_default();
                    ui.label(t.confirm_uninstall.replace("{}", &name));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if action_button(ui, t.m_uninstall.trim_end_matches('…'), true, 140.0).clicked() {
                            do_uninstall = Some(id.clone());
                            close = true;
                        }
                        if action_button(ui, t.cancel, false, 120.0).clicked() {
                            close = true;
                        }
                    });
                    return;
                }
                Dialog::Versions(id) | Dialog::Notes(id) => {
                    let Some(app) = self.app(id) else { return };
                    let notes = matches!(dialog, Dialog::Notes(_));
                    app_name(ui, &app, 18.0);
                    ui.label(RichText::new(if notes { t.m_release_notes } else { t.m_choose_version }).color(muted(ui)));
                    ui.separator();
                    let rels = self.states.get(id).and_then(|s| s.releases.as_ref());
                    match rels {
                        None => {
                            ui.spinner();
                        }
                        Some(Err(e)) => {
                            ui.label(e);
                        }
                        Some(Ok(rels)) => {
                            let inst_tag = self.cfg.installed.get(id).map(|i| i.tag.clone());
                            if !notes && self.cfg.installed.contains_key(id) {
                                ui.label(RichText::new(t.pinned_note).size(11.0).color(muted(ui)));
                            }
                            egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                                let latest_tag = self.latest(rels, &app.asset, &app.id).map(|r| r.tag_name.clone());
                                for rel in rels.iter().filter(|r| self.cfg.prereleases || !r.prerelease || notes) {
                                    let Some(asset) = rel.asset_matching(&app.asset) else { continue };
                                    ui.horizontal(|ui| {
                                        ui.label(mono(rel.version(), 13.0).strong());
                                        if let Some(d) = &rel.published_at {
                                            ui.label(RichText::new(short_date(d, self.cfg.lang)).color(muted(ui)));
                                        }
                                        if rel.prerelease {
                                            badge(ui, t.prerelease, Color32::from_rgb(0xa8, 0x55, 0xf7));
                                        }
                                        if inst_tag.as_deref() == Some(rel.tag_name.as_str()) {
                                            badge(ui, t.current, Color32::from_rgb(0x22, 0xc5, 0x5e));
                                        }
                                        if !notes {
                                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                let label = format!("{} ({})", t.install, human_size(asset.size));
                                                if action_button(ui, &label, false, 150.0).clicked() {
                                                    let pinned = latest_tag.as_deref() != Some(rel.tag_name.as_str());
                                                    todo = Some(Action::Install(id.clone(), rel.clone(), pinned));
                                                    close = true;
                                                }
                                            });
                                        }
                                    });
                                    if notes {
                                        let body = rel.body.as_deref().unwrap_or("").trim();
                                        ui.label(if body.is_empty() { t.no_notes } else { body });
                                    }
                                    ui.separator();
                                }
                            });
                        }
                    }
                }
            }
            ui.add_space(6.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if action_button(ui, t.close, true, 110.0).clicked() {
                    close = true;
                }
            });
        });
        if modal.should_close() {
            close = true;
        }
        if let Some(id) = do_uninstall {
            self.uninstall(&id);
        }
        if let Some(Action::Install(id, rel, pinned)) = todo {
            if let Some(app) = self.app(&id) {
                self.install(&app, &rel, pinned);
            }
        }
        if close {
            self.dialog = None;
        }
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let ctx = ui.ctx().clone();
        let mut changed = false;
        let mut close = false;
        let mut autostart_changed = false;
        let mut update_self = false;
        let mut quit_now = false;
        let self_latest = self.self_latest();
        let modal = egui::Modal::new(egui::Id::unique("settings")).show(&ctx, |ui| {
            ui.set_width(860.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(t.settings).size(24.0).strong());
                ui.label(mono(format!("FCDI APPS {VERSION}"), 10.0).color(muted(ui)));
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
                ui.columns(2, |cols| {
                    let section = |ui: &mut egui::Ui, title: &str, body: &mut dyn FnMut(&mut egui::Ui)| {
                        Frame::new().stroke(Stroke::new(1.0, line(ui))).inner_margin(Margin::same(14)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(mono(title, 10.0).color(muted(ui)));
                            ui.add_space(4.0);
                            body(ui);
                        });
                        ui.add_space(10.0);
                    };

                    // Colonne gauche
                    section(&mut cols[0], t.language, &mut |ui| {
                        egui::ComboBox::from_id_salt("lang")
                            .selected_text(match self.cfg.lang {
                                Lang::Fr => "Français",
                                Lang::En => "English",
                            })
                            .width(240.0)
                            .show_ui(ui, |ui| {
                                changed |= ui.selectable_value(&mut self.cfg.lang, Lang::Fr, "Français").changed();
                                changed |= ui.selectable_value(&mut self.cfg.lang, Lang::En, "English").changed();
                            });
                    });
                    section(&mut cols[0], t.s_updates, &mut |ui| {
                        ui.horizontal(|ui| {
                            ui.label(t.s_check);
                            egui::ComboBox::from_id_salt("check")
                                .selected_text(if self.cfg.auto_check { t.s_check_auto } else { t.s_check_manual })
                                .show_ui(ui, |ui| {
                                    changed |= ui.selectable_value(&mut self.cfg.auto_check, true, t.s_check_auto).changed();
                                    changed |= ui.selectable_value(&mut self.cfg.auto_check, false, t.s_check_manual).changed();
                                });
                        });
                        changed |= ui.checkbox(&mut self.cfg.auto_install, t.s_auto_install).changed();
                        changed |= ui.checkbox(&mut self.cfg.notify, t.s_notify).changed();
                        changed |= ui.checkbox(&mut self.cfg.prereleases, t.s_prerelease).changed();
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(t.s_launcher_version);
                            match &self_latest {
                                Some(rel) if net::cmp_versions(&rel.version(), VERSION) == std::cmp::Ordering::Greater => {
                                    ui.label(mono(format!("v{VERSION} -> v{}", rel.version()), 12.0));
                                }
                                _ => {
                                    ui.label(mono(format!("v{VERSION} ({})", t.s_latest), 12.0));
                                }
                            }
                        });
                        if let Some(rel) = &self_latest {
                            if net::cmp_versions(&rel.version(), VERSION) == std::cmp::Ordering::Greater && !self.self_updated {
                                if self.self_progress.is_some() {
                                    ui.spinner();
                                } else if action_button(ui, t.s_update_launcher, true, 240.0).clicked() {
                                    update_self = true;
                                }
                            }
                        }
                        if self.self_updated {
                            ui.label(RichText::new(t.s_restart).color(Color32::from_rgb(0x22, 0xc5, 0x5e)));
                        }
                    });
                    section(&mut cols[0], t.s_appearance, &mut |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            for (th, label) in [(Theme::System, t.s_system), (Theme::Dark, t.s_dark), (Theme::Light, t.s_light)] {
                                if ui.add(egui::Button::selectable(self.cfg.theme == th, mono(label, 10.5)).min_size(Vec2::new(0.0, 30.0))).clicked() {
                                    self.cfg.theme = th;
                                    apply_theme(ui.ctx(), th);
                                    changed = true;
                                }
                            }
                        });
                        ui.add_space(4.0);
                        changed |= ui.checkbox(&mut self.cfg.show_hidden, t.s_show_hidden).changed();
                    });

                    // Colonne droite
                    section(&mut cols[1], t.s_folder_title, &mut |ui| {
                        ui.label(t.s_folder);
                        let mut text = self.cfg.install_dir.display().to_string();
                        if ui.add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY)).changed() {
                            self.cfg.install_dir = text.into();
                            changed = true;
                        }
                        ui.label(t.s_folder_help);
                        ui.horizontal(|ui| {
                            if action_button(ui, t.s_browse, false, 130.0).clicked() {
                                if let Some(dir) = rfd::FileDialog::new().set_directory(&self.cfg.install_dir).pick_folder() {
                                    self.cfg.install_dir = dir;
                                    changed = true;
                                }
                            }
                            if action_button(ui, t.s_default, false, 130.0).clicked() {
                                self.cfg.install_dir = crate::config::default_install_dir();
                                changed = true;
                            }
                        });
                        ui.label(RichText::new(t.s_folder_note).size(11.0).color(muted(ui)));
                    });
                    section(&mut cols[1], t.s_background, &mut |ui| {
                        if ui.checkbox(&mut self.cfg.autostart, t.s_autostart).changed() {
                            changed = true;
                            autostart_changed = true;
                        }
                        if self.tray.is_some() {
                            changed |= ui.checkbox(&mut self.cfg.keep_in_tray, t.s_tray).changed();
                        } else {
                            ui.label(RichText::new(t.s_tray_off).size(11.0).color(muted(ui)));
                        }
                        changed |= ui.checkbox(&mut self.cfg.desktop_entries, t.s_desktop_entries).changed();
                    });
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                if action_button(ui, t.quit, false, 200.0).clicked() {
                    quit_now = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if action_button(ui, t.done, true, 110.0).clicked() {
                        close = true;
                    }
                });
            });
        });
        if modal.should_close() {
            close = true;
        }
        if autostart_changed {
            if let Err(e) = system::set_autostart(self.cfg.autostart) {
                self.dialog = Some(Dialog::Error(format!("{e:#}")));
            }
        }
        if changed {
            self.cfg.save();
            // Ajoute ou retire les raccourcis du menu selon le réglage.
            for (id, inst) in &self.cfg.installed {
                if self.cfg.desktop_entries {
                    if let Some(app) = self.catalog.apps.iter().find(|a| &a.id == id) {
                        let _ = system::write_desktop_entry(app, &inst.path);
                    }
                } else {
                    system::remove_desktop_entry(id);
                }
            }
        }
        if quit_now {
            self.quit(&ctx);
        }
        if update_self {
            self.update_self();
        }
        if close {
            self.settings_open = false;
        }
    }
}
