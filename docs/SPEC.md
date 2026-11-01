# SaveVault — décisions de conception

Récapitulatif des choix faits avant le développement (1er novembre 2026). Maquette : https://claude.ai/artifact/CTsFYuV6cVKNrED96at3H6

## Application

- App de bureau **Tauri 2** : interface **Svelte 5 + TypeScript**, couche **Rust** pour l'accès disque (scan, zip, corbeille, restauration, détection de processus).
- Usage manuel, pour un utilisateur. Dépôt public, licence MIT.
- Interface en français par défaut, anglais disponible.

## Détection

- Jeux Steam, Epic, Ubisoft Connect, GOG et jeux sans launcher, plus des chemins ajoutés à la main.
- Le **manifeste Ludusavi** est téléchargé au premier lancement, réduit aux chemins Windows tagués `save`, et mis à jour depuis les paramètres. Il n'est pas inclus dans le dépôt (licence des données PCGamingWiki).
- `Steam\userdata\<compte>\<appid>\remote` est ajouté pour tout jeu Steam ; les AppIDs non reconnus vont dans « Non identifiés ».
- Un scan des dossiers habituels (`Documents\My Games`, `Saved Games`, `AppData\LocalLow\<éditeur>\<jeu>`, `AppData\Roaming`, `AppData\Local`, `Documents`) remplit l'onglet **Non identifiés**, avec une suggestion de jeu par nom. On peut rattacher (chemin perso) ou ignorer.
- Les jeux désinstallés sont affichés avec un badge.
- Les comptes Steam sont mélangés, avec un filtre par compte (`loginusers.vdf`).
- Au démarrage : affichage du scan en cache, puis nouveau scan en arrière-plan. Bouton « Rescanner ».
- Garde-fous : jamais un dossier système, une racine de launcher ou un dossier d'installation entier comme emplacement de save.

## Images

CDN Steam (via l'AppID, y compris celui du manifeste pour un jeu hors Steam) → SteamGridDB si une clé est configurée → jaquette générée (couleur + titre). Cache sur disque.

## Actions

- **Backup** : zip horodaté contenant `savevault.json` (jeu, chemins d'origine, date, type), rangé dans `<dossier choisi>\<jeu>\`. Tout est gardé par défaut ; option « garder les N derniers par jeu » (backups manuels seulement).
- **Restaurer** : backup « avant restauration » de l'état actuel, envoi à la corbeille, extraction aux chemins d'origine. Pas de fusion.
- **Supprimer** : backup « avant suppression », puis corbeille, après confirmation (chemin, taille).
- Suppression et restauration **bloquées si le jeu tourne** (processus lancé depuis son dossier d'installation). Avertissement **Steam Cloud** pour les jeux synchronisés.
- Onglet **Backups** : vue globale, restauration et suppression.
- Le dossier de backup est choisi au premier lancement (proposition : `Documents\SaveVault\Backups`).

## Mises à jour

- GitHub Actions sur chaque push sur `main` : version via Conventional Commits, notes générées, build Windows, release + `latest.json` (`tauri-action`).
- Installeur NSIS par utilisateur (sans admin). Pas de signature de code Windows : avertissement SmartScreen accepté.
- Updater Tauri : vérification discrète au démarrage (pastille sur Paramètres) et bouton « Rechercher une mise à jour » ; installation et redémarrage au clic.

## Hors périmètre v1

Backups automatiques planifiés, émulateurs, saves dans le registre, synchronisation cloud.
