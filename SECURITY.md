# Sécurité de FCDI Apps

## Ce que le lanceur vérifie

- **Signature Ed25519** : chaque AppImage publié dans une Release doit être accompagné d'un fichier
  `<nom>.sig`. Le lanceur ne met un fichier en place que si la signature est valide pour la clé publique
  de `keys/signing.pub`. Sinon le téléchargement est supprimé et rien n'est installé.
- **Ce que la signature couvre** : application, version (étiquette de la Release), nom du fichier et
  empreinte SHA-256 du contenu (voir `src/verify.rs`). Un fichier ne peut donc pas être remplacé par un autre,
  ni rejoué sous une autre version ou une autre application.
- **Versions sans signature** : jamais proposées par le lanceur. Seule exception : la liste figée
  `LEGACY_UNSIGNED` (`src/verify.rs`) pour les applications qui n'ont pas encore de signature ; elle ne peut être
  changée que par une version signée du lanceur. Retirer un identifiant dès que l'application signe.
- **Mise à jour du lanceur** : même contrôle (application `fcdi-apps`).
- **Catalogue** (`apps.json`) : entrées invalides écartées (identifiant, dépôt, nom de fichier), textes nettoyés
  (pas de retour à la ligne : pas d'injection dans les raccourcis du bureau), icônes et sites en `https` seulement.
- **Réseau** : `https` obligatoire, y compris lors des redirections.

Même si le catalogue ou une Release était piraté, il faudrait en plus la **clé privée** pour qu'un faux fichier soit accepté.

## La clé de signature

- Création (une fois) : `tools/gen-key.sh` (clé privée dans `~/fcdi-signing.pem`, clé publique affichée).
  La clé privée reste **hors ligne, sur votre ordinateur**, et doit être sauvegardée. Ne la mettez ni sur GitHub,
  ni dans un secret d'Actions, ni dans une conversation.
- Publier une version : la CI crée la Release, puis vous signez : `tools/publish-signature.sh <dépôt> <id> <étiquette> <fichier>`
  (télécharge le fichier de la Release, affiche son SHA-256, demande confirmation, signe, ajoute le `.sig`).
- Tant que le `.sig` n'est pas publié, le lanceur n'offre pas la nouvelle version.
- Clé perdue ou compromise : publier un nouveau lanceur avec une nouvelle clé publique dans `keys/signing.pub`
  (plusieurs clés peuvent coexister, une par ligne pendant la transition). Les lanceurs déjà installés ne pourront pas
  se mettre à jour automatiquement vers une clé qu'ils ne connaissent pas : prévoir une réinstallation manuelle.

## Limites

- La signature prouve que vous avez signé ce fichier, pas qu'il est exempt de défaut : relisez ce que vous publiez.
- Si le compte GitHub est piraté avant la signature, l'attaquant peut remplacer le fichier de la Release : vérifiez
  l'empreinte affichée par `publish-signature.sh` (comparez avec celle du build dans les logs de la CI, ou signez
  un fichier construit sur votre machine).
- Protégez le compte GitHub (authentification à deux facteurs, protection de `main`, jetons limités à un dépôt).

## Signaler un problème

Ouvrir une issue privée ou écrire au propriétaire du dépôt.
