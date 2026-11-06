# Journal des versions

## 0.2.2

- Liste alignée : colonnes à largeur fixe dans l'ordre Nom / Version / Taille / Date / Statut / actions ; colonne éditeur retirée (c'est toujours FCDI).

## 0.2.1

- Le poids du fichier à télécharger est affiché dans la liste et sur les cartes (Ko, Mo ou Go).

## 0.2.0

- Sécurité : les AppImage (applications et lanceur) ne sont installés que si leur signature Ed25519 est valide
  (fichier `.sig` dans la Release, clé publique dans `keys/signing.pub`) ; empreinte SHA-256 vérifiée avant mise en place.
- Les versions sans signature ne sont plus proposées (liste figée d'exceptions : Luminux IA, le temps qu'il signe).
- Catalogue durci : entrées invalides écartées, textes nettoyés, icônes et sites en https seulement ; https obligatoire (redirections comprises).
- Outils : `tools/gen-key.sh`, `tools/sign.sh`, `tools/publish-signature.sh` ; documentation dans `SECURITY.md`.
- CI : signature automatique avec le secret `FCDI_SIGNING_KEY` ; la publication échoue si le secret est absent.

## 0.1.7

- Catalogue : Audit données perso ajouté (la première Release du dépôt devra contenir un fichier *x86_64.AppImage).

## 0.1.6

- Icône : emblème « F » dans son @, redessiné en vectoriel avec le F de la police du logo (assets/emblem-F.svg) ; bannière vectorielle (assets/banniere-FCDI.svg).

## 0.1.5

- Vrai logo FCDI : bannière haute définition et icône (roundel F) fournis par Franck.

## 0.1.4

- Catalogue : David FIT (version PC, AppImage) ajouté.

## 0.1.3

- Icône dans la zone de notification : fermer la fenêtre garde le lanceur actif (réglage « Rester actif dans la zone de notification à la fermeture de la fenêtre », activé par défaut) ; clic sur l'icône ou « Ouvrir FCDI Apps » rouvre la fenêtre, « Quitter » arrête le lanceur.
- Les vérifications de mise à jour continuent fenêtre cachée.
- Sans zone de notification sur le bureau, fermer la fenêtre quitte comme avant. Au démarrage automatique (`--minimized`), la fenêtre démarre cachée si l'icône est disponible.

## 0.1.2

- Icône de l'application : logo sur tuile blanche arrondie, plus propre que le détourage sur fond transparent (provisoire, en attendant un logo vectoriel).

## 0.1.1

- Logo FCDI : icône de l'application et en-tête (images provisoires, à remplacer par des versions de meilleure qualité).
- Mise à jour du lanceur : bandeau « Nouvelle version » sur l'écran principal, bouton Relancer après la mise à jour, et mise à jour automatique si « Télécharger et installer les mises à jour automatiquement » est coché.
- Limite de l'API GitHub : les Releases sont gardées sur disque et revérifiées par ETag (une réponse « rien de nouveau » ne compte pas dans la limite) ; en cas de limite atteinte ou hors ligne, les dernières versions connues restent affichées.

## 0.1.0

Première version.
- Liste des applications du catalogue `apps.json` avec version installée, dernière version et date.
- Installer, mettre à jour, ouvrir ; menu « … » : choisir une version, notes de version, afficher dans Fichiers, réinstaller, désinstaller, site web, dépôt GitHub, toutes les versions, masquer.
- Réglages : langue (français, anglais), mises à jour automatiques et préversions, thème, dossier d'installation des AppImage, lancement à l'ouverture de session, raccourcis dans le menu du bureau.
- Mise à jour du lanceur lui-même depuis les Releases.
