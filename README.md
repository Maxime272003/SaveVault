# SaveVault

Application Windows qui répertorie les sauvegardes de tes jeux vidéo, te permet de les ouvrir, d'en faire un backup zip, de les restaurer ou de les supprimer (toujours via la corbeille, après un backup automatique).

## Fonctionnalités

- **Détection** des saves de plus de 50 000 jeux grâce au [manifeste Ludusavi](https://github.com/mtkennerly/ludusavi-manifest), plus les dossiers `Steam\userdata` et des chemins personnalisés.
- **Launchers** : Steam (toutes les bibliothèques, tous les comptes), Epic Games, Ubisoft Connect, GOG, et des dossiers de jeux libres (`C:\Games` par défaut).
- **Registre Windows** : les saves stockées dans `HKEY_CURRENT_USER` sont détectées, sauvegardées (`.reg`) et restaurées.
- **Émulateurs** : Ryujinx, yuzu et dérivés, Dolphin, Cemu, Citra/Lime3DS/Azahar, PCSX2, DuckStation, RPCS3, PPSSPP, Vita3K, RetroArch.
- **Jeux désinstallés** : leurs saves restantes sont listées aussi, pour faire le ménage.
- **Dossiers non identifiés** : les dossiers qui ressemblent à des saves sont proposés, avec une suggestion de jeu, à rattacher ou ignorer.
- **Backups zip** horodatés (avec un `savevault.json` qui décrit les chemins d'origine), **vérifiés après création**, **restauration** en un clic, rétention optionnelle des N derniers par jeu.
- **Backup en lot** (tous les jeux, ou une sélection) et **backups automatiques** (à l'ouverture, chaque jour ou chaque semaine) des jeux modifiés.
- **Espace disque** : saves les plus lourdes, nettoyage des jeux désinstallés, poids des backups.
- **Confort** : menu clic droit, raccourcis clavier, sélection multiple, liste des fichiers d'un emplacement, barre de progression, journal des erreurs.
- **Sécurité** : backup automatique avant chaque suppression ou restauration, envoi à la corbeille, blocage si le jeu tourne, avertissement Steam Cloud.
- **Jaquettes** depuis le cache de Steam ou son CDN, sinon [SteamGridDB](https://www.steamgriddb.com/) (clé API optionnelle), mises en cache sur le disque.
- **Mises à jour intégrées** : vérification au démarrage et dans les paramètres.
- Interface en français et en anglais.

## Installation

Télécharge `SaveVault_x.y.z_x64-setup.exe` depuis la [dernière release](https://github.com/Maxime272003/SaveVault/releases/latest). L'installeur ne demande pas de droits administrateur.

Le `.exe` n'est pas signé : Windows SmartScreen affichera un avertissement la première fois (« Informations complémentaires » puis « Exécuter quand même »).

## Développement

Prérequis : [Node.js](https://nodejs.org/) 22+, [pnpm](https://pnpm.io/) 10, [Rust](https://rustup.rs/) stable et les [prérequis Tauri pour Windows](https://tauri.app/start/prerequisites/).

```bash
pnpm install
pnpm tauri dev
```

Tests :

```bash
pnpm check
cargo test --manifest-path src-tauri/Cargo.toml
```

Scan réel du disque (lecture seule), avec un manifeste téléchargé localement :

```bash
SAVEVAULT_MANIFEST=chemin/vers/manifest.yaml cargo test --manifest-path src-tauri/Cargo.toml real_scan -- --ignored --nocapture
```

### Architecture

| Dossier | Rôle |
| --- | --- |
| `src-tauri/src/manifest.rs` | Téléchargement du manifeste Ludusavi et réduction aux chemins Windows |
| `src-tauri/src/stores.rs` | Détection de Steam, Epic, Ubisoft, GOG et des jeux installés |
| `src-tauri/src/scan.rs` | Résolution des chemins, emplacements, dossiers inconnus |
| `src-tauri/src/backup.rs` | Zip, restauration, rétention, corbeille |
| `src-tauri/src/covers.rs` | Jaquettes et cache |
| `src/lib/` | Interface Svelte 5 (état, traductions, composants) |

Les données de l'app sont dans `%APPDATA%\com.maxime272003.savevault` (réglages, manifeste) et `%LOCALAPPDATA%\com.maxime272003.savevault` (cache du scan, jaquettes).

## Releases

Chaque push sur `main` lance [`.github/workflows/release.yml`](.github/workflows/release.yml) :

1. La version est calculée à partir des [Conventional Commits](https://www.conventionalcommits.org/fr/) depuis le dernier tag : `feat:` → version mineure, `fix:` / `perf:` → correctif, `feat!:` ou `BREAKING CHANGE` → majeure. Sans `feat`/`fix`, pas de release.
2. L'app est compilée, signée pour l'updater, et publiée en release GitHub avec `latest.json`.
3. Les commits `feat`/`fix` deviennent les notes de version affichées dans l'app.

Secrets GitHub nécessaires : `TAURI_SIGNING_PRIVATE_KEY` (contenu de la clé privée de l'updater) et, si la clé en a un, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

## Licence

Code sous licence [MIT](LICENSE). Les données du manifeste Ludusavi (issues de PCGamingWiki) ne sont pas incluses dans ce dépôt : l'app les télécharge au premier lancement.
