//! Détection des launchers (Steam, Epic, Ubisoft Connect, GOG) et des jeux installés.

use crate::vdf;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Store {
    Steam,
    Epic,
    Ubisoft,
    Gog,
    Emulator,
    Other,
}

#[derive(Debug, Clone)]
pub struct InstalledGame {
    pub store: Store,
    pub name: String,
    pub install_dir: PathBuf,
    pub steam_id: Option<u32>,
    pub gog_id: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct SteamInfo {
    pub root: Option<PathBuf>,
    pub libraries: Vec<PathBuf>,
    /// id32 du compte → nom affiché.
    pub users: HashMap<u32, String>,
}

#[derive(Debug, Clone, Default)]
pub struct Stores {
    pub steam: SteamInfo,
    pub ubisoft_root: Option<PathBuf>,
    pub installed: Vec<InstalledGame>,
}

const STEAM_ID64_BASE: u64 = 76_561_197_960_265_728;

pub fn steam_id64_to_id32(id64: u64) -> Option<u32> {
    id64.checked_sub(STEAM_ID64_BASE).and_then(|v| u32::try_from(v).ok())
}

impl Stores {
    pub fn detect(program_data: Option<&Path>, game_dirs: &[PathBuf]) -> Self {
        let mut stores = Stores::default();
        detect_steam(&mut stores);
        detect_ubisoft(&mut stores);
        detect_gog(&mut stores);
        if let Some(pd) = program_data {
            detect_epic(&mut stores, pd);
        }
        // En dernier : les launchers restent prioritaires pour un même jeu.
        detect_game_dirs(&mut stores, game_dirs);
        stores
    }

    /// Dossier d'installation de Steam (registre, sinon emplacement par défaut).
    pub fn steam_root() -> Option<PathBuf> {
        #[cfg(windows)]
        let from_reg = reg_string(winreg::enums::HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath")
            .map(|p| PathBuf::from(p.replace('/', "\\")));
        #[cfg(not(windows))]
        let from_reg: Option<PathBuf> = None;
        from_reg
            .filter(|p| p.exists())
            .or_else(|| Some(PathBuf::from(r"C:\Program Files (x86)\Steam")).filter(|p| p.exists()))
    }

    pub fn account_name(&self, id32: u32) -> String {
        self.steam
            .users
            .get(&id32)
            .cloned()
            .unwrap_or_else(|| id32.to_string())
    }
}

#[cfg(windows)]
fn reg_string(hive: winreg::HKEY, path: &str, value: &str) -> Option<String> {
    winreg::RegKey::predef(hive)
        .open_subkey(path)
        .ok()?
        .get_value::<String, _>(value)
        .ok()
}

#[cfg(windows)]
fn reg_subkeys(hive: winreg::HKEY, path: &str) -> Vec<(String, winreg::RegKey)> {
    let Ok(key) = winreg::RegKey::predef(hive).open_subkey(path) else {
        return Vec::new();
    };
    key.enum_keys()
        .flatten()
        .filter_map(|name| key.open_subkey(&name).ok().map(|k| (name, k)))
        .collect()
}

fn detect_steam(stores: &mut Stores) {
    let Some(root) = Stores::steam_root() else { return };

    let mut libraries = vec![root.clone()];
    if let Ok(text) = fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf")) {
        let doc = vdf::parse(&text);
        if let Some(folders) = doc.get("libraryfolders") {
            for (_, lib) in folders.entries() {
                if let Some(p) = lib.get_str("path") {
                    let p = PathBuf::from(p);
                    if p.exists() && !libraries.iter().any(|l| crate::paths::norm(l) == crate::paths::norm(&p)) {
                        libraries.push(p);
                    }
                }
            }
        }
    }

    for lib in &libraries {
        let steamapps = lib.join("steamapps");
        let Ok(entries) = fs::read_dir(&steamapps) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                continue;
            }
            let Ok(text) = fs::read_to_string(entry.path()) else { continue };
            let doc = vdf::parse(&text);
            let Some(app) = doc.get("AppState") else { continue };
            let (Some(id), Some(dir)) = (
                app.get_str("appid").and_then(|s| s.parse::<u32>().ok()),
                app.get_str("installdir"),
            ) else {
                continue;
            };
            let install_dir = steamapps.join("common").join(dir);
            if !install_dir.exists() {
                continue;
            }
            stores.installed.push(InstalledGame {
                store: Store::Steam,
                name: app.get_str("name").unwrap_or(dir).to_string(),
                install_dir,
                steam_id: Some(id),
                gog_id: None,
            });
        }
    }

    let mut users = HashMap::new();
    if let Ok(text) = fs::read_to_string(root.join("config").join("loginusers.vdf")) {
        let doc = vdf::parse(&text);
        if let Some(list) = doc.get("users") {
            for (id64, user) in list.entries() {
                let Some(id32) = id64.parse::<u64>().ok().and_then(steam_id64_to_id32) else { continue };
                let name = user
                    .get_str("PersonaName")
                    .or_else(|| user.get_str("AccountName"))
                    .unwrap_or(id64);
                users.insert(id32, name.to_string());
            }
        }
    }

    stores.steam = SteamInfo { root: Some(root), libraries, users };
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EpicItem {
    display_name: Option<String>,
    install_location: Option<String>,
}

fn detect_epic(stores: &mut Stores, program_data: &Path) {
    let dir = program_data.join(r"Epic\EpicGamesLauncher\Data\Manifests");
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if entry.path().extension().and_then(|e| e.to_str()) != Some("item") {
            continue;
        }
        let Ok(text) = fs::read_to_string(entry.path()) else { continue };
        let Ok(item) = serde_json::from_str::<EpicItem>(&text) else { continue };
        let (Some(name), Some(loc)) = (item.display_name, item.install_location) else { continue };
        let install_dir = PathBuf::from(loc);
        if install_dir.exists() {
            stores.installed.push(InstalledGame {
                store: Store::Epic,
                name,
                install_dir,
                steam_id: None,
                gog_id: None,
            });
        }
    }
}

fn detect_ubisoft(stores: &mut Stores) {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        const BASE: &str = r"SOFTWARE\WOW6432Node\Ubisoft\Launcher";
        stores.ubisoft_root = reg_string(HKEY_LOCAL_MACHINE, BASE, "InstallDir")
            .map(PathBuf::from)
            .filter(|p| p.exists());
        for (_, key) in reg_subkeys(HKEY_LOCAL_MACHINE, &format!(r"{BASE}\Installs")) {
            let Ok(dir) = key.get_value::<String, _>("InstallDir") else { continue };
            let install_dir = PathBuf::from(dir.replace('/', "\\"));
            if !install_dir.exists() {
                continue;
            }
            let name = install_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            stores.installed.push(InstalledGame {
                store: Store::Ubisoft,
                name,
                install_dir,
                steam_id: None,
                gog_id: None,
            });
        }
    }
    #[cfg(not(windows))]
    let _ = stores;
}

fn detect_gog(stores: &mut Stores) {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        for (id, key) in reg_subkeys(HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\GOG.com\Games") {
            let Ok(path) = key.get_value::<String, _>("path") else { continue };
            let install_dir = PathBuf::from(path);
            if !install_dir.exists() {
                continue;
            }
            let name = key.get_value::<String, _>("gameName").unwrap_or_else(|_| id.clone());
            stores.installed.push(InstalledGame {
                store: Store::Gog,
                name,
                install_dir,
                steam_id: None,
                gog_id: id.parse().ok(),
            });
        }
    }
    #[cfg(not(windows))]
    let _ = stores;
}

/// Chaque sous-dossier d'un dossier de jeux (ex. `C:\Games\FINAL FANTASY VII REBIRTH`) est un jeu installé.
fn detect_game_dirs(stores: &mut Stores, game_dirs: &[PathBuf]) {
    for dir in game_dirs {
        let Ok(entries) = fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let key = crate::paths::norm(&path);
            if stores.installed.iter().any(|g| crate::paths::norm(&g.install_dir) == key) {
                continue;
            }
            stores.installed.push(InstalledGame {
                store: Store::Other,
                name: entry.file_name().to_string_lossy().to_string(),
                install_dir: path,
                steam_id: None,
                gog_id: None,
            });
        }
    }
}
