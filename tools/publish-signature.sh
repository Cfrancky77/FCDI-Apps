#!/usr/bin/env bash
# Signe l'AppImage d'une Release GitHub et ajoute le fichier .sig à cette Release.
# Usage : tools/publish-signature.sh <propriétaire/dépôt> <id-application> <étiquette> <nom-du-fichier> [clé.pem]
#   ex. : tools/publish-signature.sh Cfrancky77/FCDI-Apps fcdi-apps v0.2.0 FCDI_Apps-x86_64.AppImage
# Nécessite la commande `gh` connectée à GitHub (gh auth login). La clé privée reste sur cet ordinateur.
# Avant de signer, comparez l'empreinte affichée avec celle du build si vous pouvez (voir SECURITY.md) :
# la signature atteste que VOUS avez validé ce fichier précis.
set -euo pipefail
[ $# -ge 4 ] || { sed -n '2,7p' "$0" | sed 's/^# \?//'; exit 1; }
REPO="$1"; APP="$2"; TAG="$3"; FILE="$4"; KEY="${5:-${FCDI_SIGNING_KEY_FILE:-$HOME/fcdi-signing.pem}}"
HERE="$(cd "$(dirname "$0")" && pwd)"
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
gh release download "$TAG" --repo "$REPO" --pattern "$FILE" --dir "$TMP"
echo "SHA-256 du fichier de la Release :"
sha256sum "$TMP/$FILE"
read -r -p "Signer ce fichier ? [o/N] " ok
[ "$ok" = "o" ] || [ "$ok" = "O" ] || { echo "Annulé."; exit 1; }
"$HERE/sign.sh" "$APP" "$TAG" "$TMP/$FILE" "$KEY"
gh release upload "$TAG" "$TMP/$FILE.sig" --repo "$REPO" --clobber
echo "Signature publiée sur $REPO $TAG."
