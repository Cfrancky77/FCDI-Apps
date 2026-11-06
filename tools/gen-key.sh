#!/usr/bin/env bash
# Crée la paire de clés de signature (une seule fois). Affiche la clé PUBLIQUE à donner au lanceur.
# La clé privée reste dans le fichier indiqué : à sauvegarder, et à ne jamais partager ni publier.
set -euo pipefail
KEY="${1:-$HOME/fcdi-signing.pem}"
[ -e "$KEY" ] && { echo "$KEY existe déjà : je n'écrase pas." >&2; exit 1; }
umask 077
openssl genpkey -algorithm ED25519 -out "$KEY"
echo "Clé privée créée : $KEY  (À SAUVEGARDER, ne la partagez jamais)"
echo "Clé publique (à mettre dans keys/signing.pub) :"
openssl pkey -in "$KEY" -pubout -outform DER | tail -c 32 | base64
