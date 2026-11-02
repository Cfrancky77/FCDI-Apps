# Journal des versions

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
