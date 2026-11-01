//! Scan des saves : manifeste Ludusavi + Steam userdata + chemins perso + dossiers inconnus.

use crate::config::AppConfig;
use crate::manifest::{MFile, MGame, Manifest};
use crate::paths::{self, KnownDirs};
use crate::stores::{self, InstalledGame, Store, Stores};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LocationKind {
    /// Le dossier entier est la save.
    Dir,
    /// Seuls certains fichiers du dossier sont la save.
    Files,
    /// Clé de registre HKCU (`path` = chemin de la clé).
    Registry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub path: String,
    pub kind: LocationKind,
    #[serde(default)]
    pub files: Vec<String>,
    pub size: u64,
    pub file_count: u64,
    pub modified: Option<i64>,
    #[serde(default)]
    pub custom: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub title: String,
    pub source: Store,
    pub steam_app_id: Option<u32>,
    pub installed: bool,
    pub install_dir: Option<String>,
    pub cloud_steam: bool,
    pub locations: Vec<Location>,
    pub size: u64,
    pub file_count: u64,
    pub modified: Option<i64>,
    pub accounts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnknownDir {
    pub path: String,
    pub name: String,
    pub hint: Option<String>,
    pub steam_app_id: Option<u32>,
    /// Jeu connu dont le nom correspond au dossier, proposé pour le rattachement.
    #[serde(default)]
    pub suggestion: Option<String>,
    pub size: u64,
    pub file_count: u64,
    pub modified: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub games: Vec<Game>,
    pub unidentified: Vec<UnknownDir>,
    pub scanned_at: i64,
    pub accounts: Vec<String>,
}

/// AppIDs de `userdata` qui ne sont pas des jeux (config Steam, captures, manettes…).
const STEAM_NON_GAMES: &[u32] = &[7, 760, 241100, 228980];

struct Ctx<'a> {
    dirs: &'a KnownDirs,
    stores: &'a Stores,
    protected: Vec<String>,
    installs: Vec<String>,
    user_name: String,
    dir_cache: DirCache,
    /// AppID → dossiers `Steam\userdata\<compte>\<appid>\remote`, listés une seule fois.
    steam_remote: HashMap<u32, Vec<PathBuf>>,
}

pub fn run(manifest: &Manifest, config: &AppConfig) -> ScanResult {
    let dirs = KnownDirs::detect();
    let game_dirs: Vec<PathBuf> = config.game_dirs.iter().map(PathBuf::from).collect();
    let stores = Stores::detect(dirs.program_data.as_deref(), &game_dirs);
    let mut protected: Vec<String> = dirs.protected().iter().map(|p| paths::norm(p)).collect();
    if let Some(root) = &stores.steam.root {
        protected.push(paths::norm(root));
    }
    protected.extend(stores.steam.libraries.iter().map(|p| paths::norm(p)));
    if let Some(root) = &stores.ubisoft_root {
        protected.push(paths::norm(root));
    }
    let ctx = Ctx {
        installs: stores.installed.iter().map(|g| paths::norm(&g.install_dir)).collect(),
        user_name: dirs.user_name.clone().unwrap_or_default(),
        dirs: &dirs,
        stores: &stores,
        protected,
        dir_cache: DirCache::default(),
        steam_remote: steam_remote_dirs(&stores),
    };

    let index = InstallIndex::new(&stores.installed);
    let mut games: Vec<Game> = manifest
        .games
        .par_iter()
        .filter_map(|g| scan_manifest_game(&ctx, &index, g))
        .collect();

    let mut covered: Vec<String> = covered_paths(&games);
    let mut unidentified = Vec::new();
    add_steam_userdata(&ctx, &mut games, &mut unidentified, &covered);
    add_emulators(&ctx, &mut games);
    add_custom_entries(&ctx, manifest, config, &mut games);
    covered = covered_paths(&games);

    let ignored: HashSet<String> = config.ignored_paths.iter().map(|p| paths::norm(Path::new(p))).collect();
    unidentified.extend(find_unknown_dirs(&ctx, &covered));
    unidentified.retain(|u| !ignored.contains(&paths::norm(Path::new(&u.path))));
    unidentified.sort_by(|a, b| b.modified.cmp(&a.modified));
    suggest_games(manifest, &games, &mut unidentified);

    games.sort_by(|a, b| b.modified.cmp(&a.modified));
    let mut accounts: Vec<String> = stores.steam.users.values().cloned().collect();
    accounts.sort();
    accounts.dedup();

    ScanResult {
        games,
        unidentified,
        scanned_at: chrono::Utc::now().timestamp_millis(),
        accounts,
    }
}

struct InstallIndex<'a> {
    by_steam: HashMap<u32, &'a InstalledGame>,
    by_gog: HashMap<u64, &'a InstalledGame>,
    by_folder: HashMap<String, &'a InstalledGame>,
    by_title: HashMap<String, &'a InstalledGame>,
}

fn title_key(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

impl<'a> InstallIndex<'a> {
    fn new(installed: &'a [InstalledGame]) -> Self {
        let mut idx = InstallIndex {
            by_steam: HashMap::new(),
            by_gog: HashMap::new(),
            by_folder: HashMap::new(),
            by_title: HashMap::new(),
        };
        for g in installed {
            if let Some(id) = g.steam_id {
                idx.by_steam.insert(id, g);
            }
            if let Some(id) = g.gog_id {
                idx.by_gog.insert(id, g);
            }
            if let Some(folder) = g.install_dir.file_name() {
                idx.by_folder.entry(folder.to_string_lossy().to_lowercase()).or_insert(g);
            }
            idx.by_title.entry(title_key(&g.name)).or_insert(g);
        }
        idx
    }

    fn find(&self, g: &MGame) -> Option<&'a InstalledGame> {
        g.steam_id
            .and_then(|id| self.by_steam.get(&id))
            .or_else(|| g.gog_id.and_then(|id| self.by_gog.get(&id)))
            .or_else(|| g.install_dirs.iter().find_map(|d| self.by_folder.get(&d.to_lowercase())))
            .or_else(|| self.by_title.get(&title_key(&g.name)))
            .copied()
    }
}

fn store_tag(store: Store) -> &'static str {
    match store {
        Store::Steam => "steam",
        Store::Epic => "epic",
        Store::Ubisoft => "uplay",
        Store::Gog => "gog",
        Store::Emulator | Store::Other => "other",
    }
}

fn scan_manifest_game(ctx: &Ctx, index: &InstallIndex, g: &MGame) -> Option<Game> {
    let inst = index.find(g);
    let mut matched: Vec<PathBuf> = Vec::new();
    for f in &g.files {
        for pattern in expand(ctx, f, g, inst) {
            matched.extend(glob_existing(&ctx.dir_cache, &pattern));
        }
    }
    if let Some(dirs) = g.steam_id.and_then(|id| ctx.steam_remote.get(&id)) {
        matched.extend(dirs.iter().cloned());
    }
    let registry: Vec<Location> = g.registry.iter().filter_map(|k| registry_location(k)).collect();
    if matched.is_empty() && registry.is_empty() {
        return None;
    }
    let mut locations = if matched.is_empty() { Vec::new() } else { build_locations(ctx, matched) };
    locations.extend(registry);
    if locations.is_empty() {
        return None;
    }
    Some(make_game(ctx, g.name.clone(), g.steam_id, g.cloud_steam, inst, locations))
}

fn registry_location(key: &str) -> Option<Location> {
    let count = crate::registry::entry_count(key).filter(|&n| n > 0)?;
    Some(Location {
        path: key.to_string(),
        kind: LocationKind::Registry,
        files: Vec::new(),
        size: 0,
        file_count: count,
        modified: None,
        custom: false,
    })
}

#[derive(Clone, Copy)]
enum EmuBase {
    AppData,
    Documents,
    /// Sous-dossier d'un jeu « installé » (dossiers de jeux) dont le nom contient ce texte.
    Install(&'static str),
}

/// Émulateurs connus et dossiers où ils rangent les saves de leurs jeux.
const EMULATORS: &[(&str, &[(EmuBase, &str)])] = {
    use EmuBase::*;
    &[
        ("Ryujinx (Switch)", &[(AppData, r"Ryujinx\bis\user\save"), (Install("ryujinx"), r"portable\bis\user\save")]),
        ("yuzu (Switch)", &[(AppData, r"yuzu\nand\user\save"), (Install("yuzu"), r"user\nand\user\save")]),
        ("suyu (Switch)", &[(AppData, r"suyu\nand\user\save")]),
        ("Sudachi (Switch)", &[(AppData, r"sudachi\nand\user\save")]),
        ("Citron (Switch)", &[(AppData, r"citron\nand\user\save")]),
        ("Eden (Switch)", &[(AppData, r"eden\nand\user\save")]),
        (
            "Dolphin (GameCube / Wii)",
            &[
                (Documents, r"Dolphin Emulator\GC"),
                (Documents, r"Dolphin Emulator\Wii\title"),
                (AppData, r"Dolphin Emulator\GC"),
                (AppData, r"Dolphin Emulator\Wii\title"),
                (Install("dolphin"), r"User\GC"),
                (Install("dolphin"), r"User\Wii\title"),
            ],
        ),
        ("Cemu (Wii U)", &[(AppData, r"Cemu\mlc01\usr\save"), (Install("cemu"), r"mlc01\usr\save")]),
        ("Citra (3DS)", &[(AppData, r"Citra\sdmc\Nintendo 3DS")]),
        ("Lime3DS (3DS)", &[(AppData, r"Lime3DS\sdmc\Nintendo 3DS")]),
        ("Azahar (3DS)", &[(AppData, r"Azahar\sdmc\Nintendo 3DS")]),
        ("PCSX2 (PS2)", &[(Documents, r"PCSX2\memcards"), (Install("pcsx2"), "memcards")]),
        ("DuckStation (PS1)", &[(Documents, r"DuckStation\memcards"), (Install("duckstation"), "memcards")]),
        ("RPCS3 (PS3)", &[(Install("rpcs3"), r"dev_hdd0\home\00000001\savedata")]),
        ("PPSSPP (PSP)", &[(Documents, r"PPSSPP\PSP\SAVEDATA"), (Install("ppsspp"), r"memstick\PSP\SAVEDATA")]),
        ("Vita3K (PS Vita)", &[(AppData, r"Vita3K\Vita3K\ux0\user\00\savedata")]),
        (
            "RetroArch",
            &[
                (AppData, r"RetroArch\saves"),
                (AppData, r"RetroArch\states"),
                (Install("retroarch"), "saves"),
                (Install("retroarch"), "states"),
            ],
        ),
    ]
};

fn add_emulators(ctx: &Ctx, games: &mut Vec<Game>) {
    for (name, places) in EMULATORS {
        let mut install: Option<&InstalledGame> = None;
        let mut locations: Vec<Location> = Vec::new();
        for (base, rel) in places.iter() {
            let roots: Vec<(PathBuf, Option<&InstalledGame>)> = match base {
                EmuBase::AppData => ctx.dirs.app_data.iter().map(|p| (p.clone(), None)).collect(),
                EmuBase::Documents => ctx.dirs.documents.iter().map(|p| (p.clone(), None)).collect(),
                EmuBase::Install(needle) => ctx
                    .stores
                    .installed
                    .iter()
                    .filter(|g| {
                        g.install_dir
                            .file_name()
                            .is_some_and(|n| n.to_string_lossy().to_lowercase().contains(needle))
                    })
                    .map(|g| (g.install_dir.clone(), Some(g)))
                    .collect(),
            };
            for (root, inst) in roots {
                let dir = root.join(rel);
                if dir.is_dir() {
                    let loc = dir_location(&dir, false);
                    if loc.file_count > 0 {
                        locations.push(loc);
                        install = install.or(inst);
                    }
                }
            }
        }
        if locations.is_empty() || games.iter().any(|g| g.id == *name) {
            continue;
        }
        let mut game = make_game(ctx, name.to_string(), None, false, install, locations);
        game.source = Store::Emulator;
        games.push(game);
    }
}

fn steam_remote_dirs(stores: &Stores) -> HashMap<u32, Vec<PathBuf>> {
    let mut out: HashMap<u32, Vec<PathBuf>> = HashMap::new();
    let Some(root) = &stores.steam.root else { return out };
    for user in list_dirs(&root.join("userdata")) {
        for app in list_dirs(&user) {
            let Some(id) = app.file_name().and_then(|n| n.to_str()).and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            let remote = app.join("remote");
            if remote.is_dir() {
                out.entry(id).or_default().push(remote);
            }
        }
    }
    out
}

fn make_game(
    ctx: &Ctx,
    title: String,
    steam_app_id: Option<u32>,
    cloud_steam: bool,
    inst: Option<&InstalledGame>,
    locations: Vec<Location>,
) -> Game {
    let mut game = Game {
        id: title.clone(),
        title,
        source: Store::Other,
        steam_app_id,
        installed: inst.is_some(),
        install_dir: inst.map(|i| i.install_dir.to_string_lossy().to_string()),
        cloud_steam,
        locations,
        size: 0,
        file_count: 0,
        modified: None,
        accounts: Vec::new(),
    };
    finalize_game(ctx, &mut game, inst.map(|i| i.store));
    game
}

/// Recalcule totaux, source et comptes Steam à partir des emplacements.
fn finalize_game(ctx: &Ctx, game: &mut Game, installed_store: Option<Store>) {
    game.size = game.locations.iter().map(|l| l.size).sum();
    game.file_count = game.locations.iter().map(|l| l.file_count).sum();
    game.modified = game.locations.iter().filter_map(|l| l.modified).max();
    let steam_root = ctx.stores.steam.root.as_deref().map(paths::norm);
    let ubi_root = ctx.stores.ubisoft_root.as_deref().map(paths::norm);
    let within = |root: &Option<String>| {
        root.as_ref()
            .is_some_and(|r| game.locations.iter().any(|l| paths::is_within(&paths::norm(Path::new(&l.path)), r)))
    };
    game.source = installed_store.unwrap_or_else(|| {
        if within(&steam_root) {
            Store::Steam
        } else if within(&ubi_root) {
            Store::Ubisoft
        } else {
            Store::Other
        }
    });
    let mut accounts = BTreeSet::new();
    for loc in &game.locations {
        if let Some(name) = steam_account_of(ctx, &loc.path) {
            accounts.insert(name);
        }
    }
    game.accounts = accounts.into_iter().collect();
}

fn steam_account_of(ctx: &Ctx, path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split(['\\', '/']).collect();
    for (i, part) in parts.iter().enumerate() {
        if part.eq_ignore_ascii_case("userdata") {
            if let Some(id) = parts.get(i + 1).and_then(|s| s.parse::<u32>().ok()) {
                return Some(ctx.stores.account_name(id));
            }
        }
        if part.len() == 17 && part.starts_with("7656119") {
            // Seulement pour un compte connu : certains jeux utilisent un SteamID factice.
            if let Some(name) = part
                .parse::<u64>()
                .ok()
                .and_then(stores::steam_id64_to_id32)
                .and_then(|id| ctx.stores.steam.users.get(&id))
            {
                return Some(name.clone());
            }
        }
    }
    None
}

/// Transforme un chemin du manifeste (avec `<placeholders>`) en motifs glob concrets.
fn expand(ctx: &Ctx, f: &MFile, g: &MGame, inst: Option<&InstalledGame>) -> Vec<String> {
    let p = &f.path;
    if p.contains("<xdg") || p.contains("<reg") {
        return vec![];
    }
    if p.starts_with('~') || p.starts_with("<home>/.") || p.starts_with("<home>/Library") {
        return vec![];
    }
    let esc = |path: &Path| glob::Pattern::escape(&path.to_string_lossy());
    let mut s = p.clone();
    let simple: [(&str, &Option<PathBuf>); 7] = [
        ("<winAppData>", &ctx.dirs.app_data),
        ("<winLocalAppData>", &ctx.dirs.local_app_data),
        ("<winDocuments>", &ctx.dirs.documents),
        ("<winPublic>", &ctx.dirs.public),
        ("<winProgramData>", &ctx.dirs.program_data),
        ("<winDir>", &ctx.dirs.win_dir),
        ("<home>", &ctx.dirs.home),
    ];
    for (token, value) in simple {
        if s.contains(token) {
            match value {
                Some(v) => s = s.replace(token, &esc(v)),
                None => return vec![],
            }
        }
    }
    s = s.replace("<storeUserId>", "*");
    s = s.replace("<osUserName>", &glob::Pattern::escape(&ctx.user_name));

    let uses_install = s.contains("<base>") || s.contains("<root>");
    if uses_install && !f.stores.is_empty() {
        if let Some(i) = inst {
            if !f.stores.iter().any(|st| st == store_tag(i.store)) && s.contains("<base>") {
                return vec![];
            }
        }
    }
    if s.contains("<base>") {
        match inst {
            Some(i) => s = s.replace("<base>", &esc(&i.install_dir)),
            None => return vec![],
        }
    }
    if s.contains("<game>") {
        let folder = inst
            .and_then(|i| i.install_dir.file_name().map(|n| n.to_string_lossy().to_string()))
            .or_else(|| g.install_dirs.first().cloned())
            .unwrap_or_else(|| g.name.clone());
        s = s.replace("<game>", &glob::Pattern::escape(&folder));
    }
    if s.contains("<root>") {
        let mut roots: Vec<&PathBuf> = Vec::new();
        let any = f.stores.is_empty();
        if any || f.stores.iter().any(|st| st == "steam") {
            roots.extend(ctx.stores.steam.libraries.iter());
        }
        if any || f.stores.iter().any(|st| st == "uplay") {
            roots.extend(ctx.stores.ubisoft_root.iter());
        }
        return roots.into_iter().map(|r| s.replace("<root>", &esc(r))).collect();
    }
    if s.contains('<') && s.contains('>') {
        // Placeholder inconnu : on ne devine pas.
        return vec![];
    }
    vec![s]
}

/// Contenu des dossiers déjà lus, partagé entre threads : chaque dossier n'est listé qu'une fois
/// au lieu d'un appel disque par chemin du manifeste (~150 000 chemins).
#[derive(Default)]
struct DirCache {
    children: RwLock<HashMap<String, Option<Arc<HashSet<String>>>>>,
}

impl DirCache {
    fn list(&self, dir: &str) -> Option<Arc<HashSet<String>>> {
        if let Some(hit) = self.children.read().unwrap().get(dir) {
            return hit.clone();
        }
        let listed = fs::read_dir(dir)
            .ok()
            .map(|it| Arc::new(it.flatten().map(|e| e.file_name().to_string_lossy().to_lowercase()).collect()));
        self.children.write().unwrap().insert(dir.to_string(), listed.clone());
        listed
    }

    /// Le chemin (absolu, séparateurs `\`, sans joker) existe-t-il ? Insensible à la casse, comme Windows.
    fn exists(&self, path: &str) -> bool {
        let lower = path.to_lowercase();
        let mut parts = lower.split('\\').filter(|p| !p.is_empty() && *p != ".");
        let Some(drive) = parts.next() else { return false };
        let mut cur = format!("{drive}\\");
        for part in parts {
            match self.list(&cur) {
                Some(set) if set.contains(part) => {
                    if !cur.ends_with('\\') {
                        cur.push('\\');
                    }
                    cur.push_str(part);
                }
                _ => return false,
            }
        }
        true
    }
}

fn glob_existing(cache: &DirCache, pattern: &str) -> Vec<PathBuf> {
    let pat = pattern.replace('/', "\\");
    match pat.find(['*', '?', '[']) {
        None => {
            if cache.exists(&pat) { vec![PathBuf::from(pat)] } else { vec![] }
        }
        Some(i) => {
            let dir = pat[..i].rfind('\\').map(|j| &pat[..j]).unwrap_or("");
            if dir.is_empty() || !cache.exists(dir) {
                return vec![];
            }
            let opts = glob::MatchOptions {
                case_sensitive: false,
                require_literal_separator: true,
                require_literal_leading_dot: false,
            };
            glob::glob_with(&pat, opts)
                .map(|it| it.flatten().collect())
                .unwrap_or_default()
        }
    }
}

fn build_locations(ctx: &Ctx, matched: Vec<PathBuf>) -> Vec<Location> {
    let mut items: Vec<(String, PathBuf)> = matched.into_iter().map(|p| (paths::norm(&p), p)).collect();
    items.sort_by(|a, b| a.0.len().cmp(&b.0.len()).then(a.0.cmp(&b.0)));
    items.dedup_by(|a, b| a.0 == b.0);

    let mut kept_dirs: Vec<(String, PathBuf)> = Vec::new();
    let mut files_by_parent: HashMap<String, (PathBuf, Vec<String>)> = HashMap::new();
    for (key, path) in items {
        // Jamais un dossier système, une racine de launcher ou un dossier d'installation entier.
        if ctx.protected.iter().any(|p| paths::is_within(p, &key))
            || ctx.installs.iter().any(|i| paths::is_within(i, &key))
        {
            continue;
        }
        if kept_dirs.iter().any(|(d, _)| paths::is_within(&key, d)) {
            continue;
        }
        if path.is_dir() {
            kept_dirs.push((key, path));
        } else if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
            let entry = files_by_parent
                .entry(paths::norm(parent))
                .or_insert_with(|| (parent.to_path_buf(), Vec::new()));
            entry.1.push(name.to_string_lossy().to_string());
        }
    }
    let mut out: Vec<Location> = kept_dirs
        .into_iter()
        .map(|(_, p)| dir_location(&p, false))
        .collect();
    for (key, (parent, mut names)) in files_by_parent {
        if out.iter().any(|l| paths::is_within(&key, &paths::norm(Path::new(&l.path)))) {
            continue;
        }
        names.sort();
        names.dedup();
        out.push(files_location(&parent, names));
    }
    out.retain(|l| l.file_count > 0);
    out
}

fn to_millis(t: std::time::SystemTime) -> Option<i64> {
    t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_millis() as i64)
}

pub fn dir_location(path: &Path, custom: bool) -> Location {
    let (size, count, modified) = dir_stats(path);
    Location {
        path: path.to_string_lossy().to_string(),
        kind: LocationKind::Dir,
        files: Vec::new(),
        size,
        file_count: count,
        modified,
        custom,
    }
}

fn files_location(parent: &Path, names: Vec<String>) -> Location {
    let mut size = 0;
    let mut count = 0;
    let mut modified = None;
    for n in &names {
        if let Ok(meta) = fs::metadata(parent.join(n)) {
            if meta.is_file() {
                size += meta.len();
                count += 1;
                modified = modified.max(meta.modified().ok().and_then(to_millis));
            }
        }
    }
    Location {
        path: parent.to_string_lossy().to_string(),
        kind: LocationKind::Files,
        files: names,
        size,
        file_count: count,
        modified,
        custom: false,
    }
}

pub fn dir_stats(path: &Path) -> (u64, u64, Option<i64>) {
    let mut size = 0;
    let mut count = 0;
    let mut modified = None;
    for entry in walkdir::WalkDir::new(path).follow_links(false).into_iter().flatten() {
        if entry.file_type().is_file() {
            if let Ok(meta) = entry.metadata() {
                size += meta.len();
                count += 1;
                modified = modified.max(meta.modified().ok().and_then(to_millis));
            }
        }
    }
    (size, count, modified)
}

fn covered_paths(games: &[Game]) -> Vec<String> {
    games
        .iter()
        .flat_map(|g| g.locations.iter().map(|l| paths::norm(Path::new(&l.path))))
        .collect()
}

fn overlaps(key: &str, covered: &[String]) -> bool {
    covered.iter().any(|c| paths::is_within(key, c) || paths::is_within(c, key))
}

/// Dossiers `Steam\userdata\<compte>\<appid>\remote` qu'aucun jeu du manifeste n'a réclamés.
fn add_steam_userdata(ctx: &Ctx, games: &mut Vec<Game>, unidentified: &mut Vec<UnknownDir>, covered: &[String]) {
    let Some(root) = &ctx.stores.steam.root else { return };
    let installed: HashMap<u32, &InstalledGame> = ctx
        .stores
        .installed
        .iter()
        .filter_map(|g| g.steam_id.map(|id| (id, g)))
        .collect();
    let mut found: HashMap<u32, Vec<PathBuf>> = HashMap::new();
    let Ok(users) = fs::read_dir(root.join("userdata")) else { return };
    for user in users.flatten() {
        let Ok(apps) = fs::read_dir(user.path()) else { continue };
        for app in apps.flatten() {
            let Some(id) = app.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else { continue };
            if STEAM_NON_GAMES.contains(&id) {
                continue;
            }
            let remote = app.path().join("remote");
            if remote.is_dir() && !overlaps(&paths::norm(&remote), covered) {
                found.entry(id).or_default().push(remote);
            }
        }
    }
    for (id, dirs) in found {
        let locations: Vec<Location> = dirs
            .iter()
            .map(|d| dir_location(d, false))
            .filter(|l| l.file_count > 0)
            .collect();
        if locations.is_empty() {
            continue;
        }
        match installed.get(&id) {
            Some(inst) if !games.iter().any(|g| g.id == inst.name) => {
                games.push(make_game(ctx, inst.name.clone(), Some(id), true, Some(inst), locations));
            }
            _ => {
                for loc in locations {
                    unidentified.push(UnknownDir {
                        name: format!("Steam AppID {id}"),
                        hint: steam_account_of(ctx, &loc.path),
                        steam_app_id: Some(id),
                        suggestion: None,
                        path: loc.path,
                        size: loc.size,
                        file_count: loc.file_count,
                        modified: loc.modified,
                    });
                }
            }
        }
    }
}

fn add_custom_entries(ctx: &Ctx, manifest: &Manifest, config: &AppConfig, games: &mut Vec<Game>) {
    if config.custom_entries.is_empty() {
        return;
    }
    let by_name: HashMap<String, &MGame> = manifest.games.iter().map(|g| (g.name.to_lowercase(), g)).collect();
    for entry in &config.custom_entries {
        let path = PathBuf::from(&entry.path);
        if !path.exists() {
            continue;
        }
        let loc = if path.is_dir() {
            dir_location(&path, true)
        } else {
            let mut l = files_location(
                path.parent().unwrap_or(Path::new("")),
                vec![path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()],
            );
            l.custom = true;
            l
        };
        let key = paths::norm(&path);
        match games.iter_mut().find(|g| g.title.eq_ignore_ascii_case(&entry.game)) {
            Some(game) => {
                if !game.locations.iter().any(|l| paths::norm(Path::new(&l.path)) == key) {
                    game.locations.push(loc);
                    let store = game.installed.then_some(game.source);
                    finalize_game(ctx, game, store);
                }
            }
            None => {
                let m = by_name.get(&entry.game.to_lowercase());
                games.push(make_game(
                    ctx,
                    m.map(|m| m.name.clone()).unwrap_or_else(|| entry.game.clone()),
                    m.and_then(|m| m.steam_id),
                    m.is_some_and(|m| m.cloud_steam),
                    None,
                    vec![loc],
                ));
            }
        }
    }
}

const DENY: &[&str] = &[
    "microsoft", "google", "mozilla", "adobe", "nvidia", "nvidia corporation", "amd", "intel", "packages",
    "programs", "temp", "npm", "npm-cache", "code", "discord", "spotify", "steam", "epicgameslauncher",
    "epic", "ubisoft game launcher", "unrealengine", "unity", "unity technologies", "crashdumps",
    "d3dscache", "jetbrains", "python", "pip", "node-gyp", "pnpm", "pnpm-cache", "yarn", "electron", "slack",
    "zoom", "onedrive", "packagecache", "connecteddevicesplatform", "comms", "publishers", "virtualstore",
    "squirreltemp", "nuget", "docker", "github desktop", "gitkraken", "obs-studio", "claude", "anthropic",
    "cursor", "opera software", "brave software", "vivaldi", "battle.net", "blizzard entertainment",
    "ea desktop", "electronic arts", "origin", "gog.com", "galaxyclient", "rockstar games",
    "riot games", "overwolf", "medal", "parsec", "history", "my games", "savevault",
    "com.maxime272003.savevault", "sun", "oracle", "apple", "apple computer", "mozilla firefox",
    "thunderbird", "whatsapp", "telegram desktop", "signal", "notion", "figma", "postman", "dbeaverdata",
    "jedi", "vlc", "7-zip", "winrar", "chocolatey", "rustup", "cargo", "go", "gradle", "android",
    "virtualbox", "vmware", "wsl", "powershell", "windowsapps", "fontcache", "iconcache", "elevateddiagnostics",
    "claude-cli-nodejs", "crashreportclient", "microsoft_corporation", "epiconlineservicesuihelper",
    "epiconlineservices", "unrealengine", "nvidia corporation", "steelseries", "razer", "logitech", "corsair",
];

fn list_dirs(path: &Path) -> Vec<PathBuf> {
    fs::read_dir(path)
        .map(|it| it.flatten().filter(|e| e.path().is_dir()).map(|e| e.path()).collect())
        .unwrap_or_default()
}

fn denied(path: &Path) -> bool {
    path.file_name()
        .map(|n| {
            let n = n.to_string_lossy().to_lowercase();
            n.starts_with('.') || DENY.contains(&n.as_str())
        })
        .unwrap_or(true)
}

fn looks_like_save(dir: &Path) -> bool {
    const EXT: &[&str] = &["sav", "save", "sl2", "ess", "savx", "sv", "sfs"];
    walkdir::WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .take(3000)
        .flatten()
        .any(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.contains("save")
                || Path::new(&name)
                    .extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| EXT.contains(&x))
        })
}

/// Propose un jeu pour chaque dossier inconnu dont le nom (ou l'AppID Steam) correspond.
fn suggest_games(manifest: &Manifest, games: &[Game], unknown: &mut [UnknownDir]) {
    let mut by_key: HashMap<String, &str> = HashMap::new();
    let mut by_steam: HashMap<u32, &str> = HashMap::new();
    for g in &manifest.games {
        by_key.entry(title_key(&g.name)).or_insert(&g.name);
        if let Some(id) = g.steam_id {
            by_steam.entry(id).or_insert(&g.name);
        }
    }
    // Les jeux déjà trouvés passent devant le reste du manifeste.
    for g in games {
        by_key.insert(title_key(&g.title), &g.title);
    }
    for u in unknown.iter_mut() {
        let folder = u.name.rsplit(" / ").next().unwrap_or(&u.name);
        let key = title_key(folder.trim_end_matches(" Demo"));
        u.suggestion = u
            .steam_app_id
            .and_then(|id| by_steam.get(&id))
            .or_else(|| (key.len() >= 3).then(|| by_key.get(&key)).flatten())
            .map(|s| s.to_string());
    }
}

#[cfg(test)]
mod tests {
    /// Scan réel du disque : `SAVEVAULT_MANIFEST=chemin/manifest.yaml cargo test real_scan -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_scan() {
        let path = std::env::var("SAVEVAULT_MANIFEST").expect("SAVEVAULT_MANIFEST");
        let t = std::time::Instant::now();
        let manifest = crate::manifest::from_yaml(&std::fs::read_to_string(path).unwrap()).unwrap();
        println!("manifeste : {} jeux en {:?}", manifest.games.len(), t.elapsed());
        let t = std::time::Instant::now();
        let r = super::run(&manifest, &crate::config::AppConfig::default());
        println!("scan : {} jeux, {} inconnus en {:?}", r.games.len(), r.unidentified.len(), t.elapsed());
        for g in &r.games {
            println!("  [{:?}{}] {} — {} o, {} loc, comptes {:?}", g.source, if g.installed { "" } else { ", non installé" }, g.title, g.size, g.locations.len(), g.accounts);
            for l in &g.locations {
                println!("      {:?} {}", l.kind, l.path);
            }
        }
        for u in &r.unidentified {
            println!("  ? {} — {} -> {:?}", u.name, u.path, u.suggestion);
        }
    }
}

fn find_unknown_dirs(ctx: &Ctx, covered: &[String]) -> Vec<UnknownDir> {
    // (dossier, nom affiché, exige un fichier qui « ressemble » à une save)
    let mut candidates: Vec<(PathBuf, String, bool)> = Vec::new();
    let named = |p: &Path| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if let Some(docs) = &ctx.dirs.documents {
        for d in list_dirs(&docs.join("My Games")) {
            candidates.push((d.clone(), named(&d), false));
        }
        for d in list_dirs(docs).into_iter().filter(|d| !denied(d)) {
            candidates.push((d.clone(), named(&d), true));
        }
    }
    if let Some(sg) = &ctx.dirs.saved_games {
        for d in list_dirs(sg) {
            candidates.push((d.clone(), named(&d), false));
        }
    }
    if let Some(low) = &ctx.dirs.local_low {
        for publisher in list_dirs(low).into_iter().filter(|d| !denied(d)) {
            for d in list_dirs(&publisher) {
                let name = format!("{} / {}", named(&publisher), named(&d));
                candidates.push((d, name, false));
            }
        }
    }
    for base in [&ctx.dirs.app_data, &ctx.dirs.local_app_data].into_iter().flatten() {
        for d in list_dirs(base).into_iter().filter(|d| !denied(d)) {
            candidates.push((d.clone(), named(&d), true));
        }
    }

    candidates
        .into_par_iter()
        .filter_map(|(path, name, needs_hint)| {
            let key = paths::norm(&path);
            if overlaps(&key, covered) || ctx.installs.iter().any(|i| paths::is_within(&key, i)) {
                return None;
            }
            if needs_hint && !looks_like_save(&path) {
                return None;
            }
            let (size, file_count, modified) = dir_stats(&path);
            (file_count > 0).then(|| UnknownDir {
                path: path.to_string_lossy().to_string(),
                name,
                hint: None,
                steam_app_id: None,
                suggestion: None,
                size,
                file_count,
                modified,
            })
        })
        .collect()
}
