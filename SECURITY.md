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

- Création (une fois) : `tools/gen-key.sh` (clé privée dans `~/fcdi-signing.pem`, clé publique affichée et placée dans `keys/signing.pub`).
  Sauvegardez la clé privée ; ne l'envoyez jamais dans une conversation ni dans un dépôt.
- **Signature automatique (mode actuel)** : le contenu du fichier `.pem` est enregistré comme secret GitHub
  `FCDI_SIGNING_KEY` dans chaque dépôt qui publie des AppImage (Settings > Secrets and variables > Actions).
  La CI signe le fichier avant de créer la Release et joint le `.sig`. Sans le secret, la publication échoue exprès.
- **Signature à la main (option plus sûre)** : ne pas mettre la clé dans GitHub, publier la Release puis lancer
  `tools/publish-signature.sh <dépôt> <id> <étiquette> <fichier>` sur votre ordinateur. Un piratage du compte GitHub ne
  permet alors pas de signer.
- Tant que le `.sig` n'est pas publié, le lanceur n'offre pas la nouvelle version.
- Clé perdue ou compromise : publier un nouveau lanceur avec une nouvelle clé publique dans `keys/signing.pub`
  (plusieurs clés peuvent coexister, une par ligne pendant la transition), et supprimer l'ancien secret. Les lanceurs déjà
  installés ne pourront pas se mettre à jour automatiquement vers une clé qu'ils ne connaissent pas : prévoir une
  réinstallation manuelle.

## Limites

- La signature prouve que vous avez signé ce fichier, pas qu'il est exempt de défaut : relisez ce que vous publiez.
- Avec la clé en secret GitHub, quelqu'un qui contrôle le compte ou le dépôt (ou qui y pousse un workflow) peut faire signer
  un faux fichier : la signature protège alors contre un fichier corrompu ou remplacé en route, pas contre un compte
  piraté. La signature à la main ferme ce cas (et vérifiez alors l'empreinte affichée par `publish-signature.sh`).
- Protégez le compte GitHub (authentification à deux facteurs, protection de `main`, jetons limités à un dépôt).

## Signaler un problème

Ouvrir une issue privée ou écrire au propriétaire du dépôt.
