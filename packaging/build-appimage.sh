#!/usr/bin/env bash
# Construit FCDI_Apps-x86_64.AppImage à partir des sources.
# Prérequis : Rust (rustup) et appimagetool dans le PATH ou dans packaging/
# (https://github.com/AppImage/appimagetool/releases).
set -euo pipefail
cd "$(dirname "$0")/.."
TOOL=${APPIMAGETOOL:-$(command -v appimagetool || echo packaging/appimagetool)}

cargo build --release
APPDIR=$(mktemp -d)/FCDI_Apps.AppDir
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/256x256/apps"
cp target/release/fcdi-apps "$APPDIR/usr/bin/"
cp packaging/fcdi-apps.desktop "$APPDIR/" && cp packaging/fcdi-apps.desktop "$APPDIR/usr/share/applications/"
cp packaging/fcdi-apps.png "$APPDIR/" && cp packaging/fcdi-apps.png "$APPDIR/usr/share/icons/hicolor/256x256/apps/"
ln -s usr/bin/fcdi-apps "$APPDIR/AppRun"
# X11/Wayland, OpenGL et le pilote graphique viennent du système.
ARCH=x86_64 "$TOOL" --appimage-extract-and-run "$APPDIR" FCDI_Apps-x86_64.AppImage
echo "Créé : $(pwd)/FCDI_Apps-x86_64.AppImage"
