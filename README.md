# FCDI Apps

Lanceur des applications FCDI pour Linux : il liste les applications, montre leur version, les télécharge depuis
les Releases GitHub, les met à jour et les lance d'un clic.

## Installer

1. Télécharge `FCDI_Apps-x86_64.AppImage` dans les [Releases](../../releases/latest).
2. Lance-la :

       chmod +x FCDI_Apps-x86_64.AppImage
       ./FCDI_Apps-x86_64.AppImage

Les applications installées vont par défaut dans `~/Applications` (réglable dans Réglages → Dossier d'installation).
Les réglages sont dans `~/.config/fcdi-apps/config.json`.

## Ajouter une application

Ajouter une entrée dans [`apps.json`](apps.json) sur la branche `main` : le lanceur relit ce fichier à chaque
vérification, sans nouvelle version du lanceur.

| Champ | Rôle |
|---|---|
| `id` | identifiant stable, sans espace (ne plus le changer ensuite) |
| `name`, `accent` | nom affiché ; `accent` est collé au nom, en couleur (mettre une espace devant pour séparer : `" IA"`) |
| `description`, `description_en` | une ligne en français, en anglais |
| `publisher` | éditeur affiché |
| `repo` | dépôt GitHub `propriétaire/nom` qui publie des Releases |
| `asset` | fin du nom du fichier à télécharger dans la Release, ex. `x86_64.AppImage` |
| `icon` | adresse d'une image PNG |
| `website` | lien « Site web » du menu |
| `color` | couleur d'accent `#RRGGBB` |

Le dépôt de l'application doit être public (ou ses Releases accessibles sans connexion).

## Publier une nouvelle version du lanceur

Augmenter `version` dans `Cargo.toml`, ajouter une section dans `CHANGELOG.md`, puis fusionner dans `main` :
GitHub Actions compile l'AppImage et crée la Release `vX.Y.Z`. Les lanceurs installés la proposent dans les Réglages.

## Compiler soi-même

    packaging/build-appimage.sh

Limite de l'API GitHub : 60 requêtes par heure sans compte. Pour plus, lancer avec une variable `GITHUB_TOKEN`.
