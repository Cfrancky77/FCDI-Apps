#!/usr/bin/env bash
# Signe un fichier de Release pour FCDI Apps : écrit <fichier>.sig (signature Ed25519 en base64).
# Usage : tools/sign.sh <id-application> <étiquette-de-la-release> <fichier> [clé-privée.pem]
#   ex. : tools/sign.sh david-fit v0.1.37 DavidFIT-x86_64.AppImage ~/fcdi-signing.pem
# La clé privée n'est jamais envoyée nulle part. Le message signé doit rester identique à src/verify.rs.
set -euo pipefail
[ $# -ge 3 ] || { sed -n '2,5p' "$0" | sed 's/^# \?//'; exit 1; }
APP="$1"; TAG="$2"; FILE="$3"; KEY="${4:-${FCDI_SIGNING_KEY_FILE:-$HOME/fcdi-signing.pem}}"
[ -f "$FILE" ] || { echo "Fichier introuvable : $FILE" >&2; exit 1; }
[ -f "$KEY" ]  || { echo "Clé privée introuvable : $KEY" >&2; exit 1; }
NAME="$(basename "$FILE")"
SHA="$(sha256sum "$FILE" | cut -d' ' -f1)"
MSG="$(mktemp)"; trap 'rm -f "$MSG"' EXIT
printf 'FCDI-Apps signature v1\n%s\n%s\n%s\n%s\n' "$APP" "$TAG" "$NAME" "$SHA" > "$MSG"
openssl pkeyutl -sign -rawin -inkey "$KEY" -in "$MSG" | base64 -w0 > "$FILE.sig"
echo "Signature écrite : $FILE.sig"
