mod backup;
mod config;
mod covers;
mod manifest;
mod paths;
mod process;
mod registry;
mod scan;
mod stores;
mod vdf;

use backup::{BackupInfo, BackupKind, MetaLocation};
use config::AppConfig;
use manifest::{Manifest, ManifestFiles};
use scan::ScanResult;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

type CmdResult<T> = Result<T, String>;

/// Message d'erreur pour l'interface, aussi écrit dans le journal.
fn err(e: impl std::fmt::Display) -> String {
    let msg = format!("{e:#}");
    log::error!("{msg}");
    msg
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    task: &'static str,
    label: String,
    done: u64,
    total: u64,
    finished: bool,
}

/// Envoie la progression d'une tâche longue à l'interface (événement `progress`),
/// au plus une fois par pourcent.
struct Reporter {
    app: AppHandle,
    task: &'static str,
    label: Mutex<String>,
    last: Mutex<i64>,
}

impl Reporter {
    fn new(app: &AppHandle, task: &'static str, label: impl Into<String>) -> Self {
        Self { app: app.clone(), task, label: Mutex::new(label.into()), last: Mutex::new(-1) }
    }

    fn set_label(&self, label: impl Into<String>) {
        *self.label.lock().unwrap() = label.into();
        *self.last.lock().unwrap() = -1;
    }

    fn update(&self, done: u64, total: u64) {
        let pct = if total == 0 { 0 } else { (done * 100 / total) as i64 };
        let mut last = self.last.lock().unwrap();
        if pct == *last {
            return;
        }
        *last = pct;
        self.send(done, total, false);
    }

    fn send(&self, done: u64, total: u64, finished: bool) {
        let label = self.label.lock().unwrap().clone();
        let _ = self.app.emit("progress", ProgressEvent { task: self.task, label, done, total, finished });
    }
}

impl Drop for Reporter {
    fn drop(&mut self) {
        self.send(0, 0, true);
    }
}

struct AppState {
    config_path: PathBuf,
    cache_dir: PathBuf,
    manifest_files: Arc<ManifestFiles>,
    config: Mutex<AppConfig>,
    manifest: RwLock<Option<Arc<Manifest>>>,
    scan: Mutex<Option<ScanResult>>,
}

impl AppState {
    fn covers_dir(&self) -> PathBuf {
        self.cache_dir.join("covers")
    }

    fn scan_file(&self) -> PathBuf {
        self.cache_dir.join("scan.json")
    }

    fn config(&self) -> AppConfig {
        self.config.lock().unwrap().clone()
    }

    fn backup_dir(&self) -> CmdResult<PathBuf> {
        self.config()
            .backup_dir
            .map(PathBuf::from)
            .ok_or_else(|| "no_backup_dir".to_string())
    }

    fn game(&self, id: &str) -> CmdResult<scan::Game> {
        self.scan
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|s| s.games.iter().find(|g| g.id == id).cloned())
            .ok_or_else(|| "game_not_found".to_string())
    }

    fn ensure_not_running(&self, game_id: &str) -> CmdResult<()> {
        if let Ok(game) = self.game(game_id) {
            if let Some(dir) = &game.install_dir {
                if process::is_running_from(Path::new(dir)) {
                    return Err("game_running".into());
                }
            }
        }
        Ok(())
    }

    fn store_scan(&self, result: ScanResult) {
        if let Ok(bytes) = serde_json::to_vec(&result) {
            let _ = fs::create_dir_all(&self.cache_dir);
            let _ = fs::write(self.scan_file(), bytes);
        }
        *self.scan.lock().unwrap() = Some(result);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestStatus {
    loaded: bool,
    game_count: usize,
    updated_at: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TitleHit {
    name: String,
    steam_id: Option<u32>,
}

#[tauri::command]
fn get_config(state: State<'_, AppState>) -> AppConfig {
    state.config()
}

#[tauri::command]
fn save_config(state: State<'_, AppState>, config: AppConfig) -> CmdResult<()> {
    config.save(&state.config_path).map_err(err)?;
    *state.config.lock().unwrap() = config;
    Ok(())
}

fn manifest_status(state: &AppState) -> ManifestStatus {
    let meta = state.manifest_files.load_meta();
    let loaded = state.manifest.read().unwrap().as_ref().map(|m| m.games.len());
    ManifestStatus {
        loaded: loaded.is_some(),
        game_count: loaded.unwrap_or(meta.game_count),
        updated_at: meta.updated_at,
    }
}

#[tauri::command]
fn get_manifest_status(state: State<'_, AppState>) -> ManifestStatus {
    manifest_status(&state)
}

async fn download_manifest(app: &AppHandle, state: &AppState, force: bool) -> CmdResult<Option<Manifest>> {
    let reporter = Reporter::new(app, "manifest", "Ludusavi");
    let result = manifest::download(&state.manifest_files, force, &|done, total| reporter.update(done, total)).await;
    Ok(result.map_err(err)?.map(|(m, _)| m))
}

#[tauri::command]
async fn update_manifest(app: AppHandle, state: State<'_, AppState>, force: bool) -> CmdResult<ManifestStatus> {
    if let Some(m) = download_manifest(&app, &state, force).await? {
        *state.manifest.write().unwrap() = Some(Arc::new(m));
    }
    Ok(manifest_status(&state))
}

async fn ensure_manifest(app: &AppHandle, state: &AppState) -> CmdResult<Arc<Manifest>> {
    if let Some(m) = state.manifest.read().unwrap().clone() {
        return Ok(m);
    }
    let files = state.manifest_files.clone();
    let loaded = tauri::async_runtime::spawn_blocking(move || files.load()).await.map_err(err)?;
    let m = match loaded {
        Some(m) => m,
        None => download_manifest(app, state, true)
            .await?
            .ok_or_else(|| "manifest_unavailable".to_string())?,
    };
    let m = Arc::new(m);
    *state.manifest.write().unwrap() = Some(m.clone());
    Ok(m)
}

#[tauri::command(async)]
fn get_cached_scan(state: State<'_, AppState>) -> CmdResult<Option<ScanResult>> {
    let mut guard = state.scan.lock().unwrap();
    if guard.is_none() {
        *guard = fs::read(state.scan_file()).ok().and_then(|b| serde_json::from_slice(&b).ok());
    }
    Ok(guard.clone())
}

#[tauri::command]
async fn run_scan(app: AppHandle, state: State<'_, AppState>) -> CmdResult<ScanResult> {
    let manifest = ensure_manifest(&app, &state).await?;
    let config = state.config();
    let started = std::time::Instant::now();
    let result = tauri::async_runtime::spawn_blocking(move || scan::run(&manifest, &config))
        .await
        .map_err(err)?;
    log::info!(
        "scan : {} jeux, {} dossiers inconnus en {:?}",
        result.games.len(),
        result.unidentified.len(),
        started.elapsed()
    );
    state.store_scan(result.clone());
    Ok(result)
}

fn backup_game(dir: &Path, config: &AppConfig, game: &scan::Game, kind: BackupKind, reporter: &Reporter) -> anyhow::Result<BackupInfo> {
    let locs: Vec<MetaLocation> = game.locations.iter().map(MetaLocation::from).collect();
    let info = backup::create(dir, &game.title, &locs, kind, &|done, total| reporter.update(done, total))?;
    if config.retention_enabled {
        backup::apply_retention(dir, &game.title, config.retention_count)?;
    }
    log::info!("backup créé : {}", info.path);
    Ok(info)
}

#[tauri::command]
async fn create_backup(app: AppHandle, state: State<'_, AppState>, game_id: String) -> CmdResult<BackupInfo> {
    let game = state.game(&game_id)?;
    let dir = state.backup_dir()?;
    let config = state.config();
    tauri::async_runtime::spawn_blocking(move || {
        let reporter = Reporter::new(&app, "backup", game.title.clone());
        backup_game(&dir, &config, &game, BackupKind::Manual, &reporter)
    })
    .await
    .map_err(err)?
    .map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchError {
    game: String,
    error: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchReport {
    created: usize,
    skipped: usize,
    failed: Vec<BatchError>,
}

/// Backup de plusieurs jeux à la suite. `only_changed` saute les jeux dont aucune save
/// n'a été modifiée depuis leur dernier backup.
#[tauri::command]
async fn backup_games(
    app: AppHandle,
    state: State<'_, AppState>,
    game_ids: Vec<String>,
    only_changed: bool,
    auto: bool,
) -> CmdResult<BatchReport> {
    let dir = state.backup_dir()?;
    let config = state.config();
    let games: Vec<scan::Game> = game_ids.iter().filter_map(|id| state.game(id).ok()).collect();
    tauri::async_runtime::spawn_blocking(move || {
        let latest: std::collections::HashMap<String, i64> =
            backup::list(&dir).into_iter().fold(Default::default(), |mut m, b| {
                let e = m.entry(b.game).or_insert(0);
                *e = (*e).max(b.created_at);
                m
            });
        let reporter = Reporter::new(&app, "batch", "");
        let mut report = BatchReport { created: 0, skipped: 0, failed: Vec::new() };
        let total = games.len();
        for (i, game) in games.iter().enumerate() {
            let unchanged = latest.get(&game.title).is_some_and(|&t| t >= game.modified.unwrap_or(0));
            if only_changed && unchanged {
                report.skipped += 1;
                continue;
            }
            reporter.set_label(format!("{} ({}/{total})", game.title, i + 1));
            let kind = if auto { BackupKind::Auto } else { BackupKind::Manual };
            match backup_game(&dir, &config, game, kind, &reporter) {
                Ok(_) => report.created += 1,
                Err(e) => report.failed.push(BatchError { game: game.title.clone(), error: err(e) }),
            }
        }
        report
    })
    .await
    .map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileEntry {
    path: String,
    size: u64,
    modified: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileList {
    files: Vec<FileEntry>,
    truncated: bool,
}

/// Fichiers d'un emplacement (chemins relatifs), pour les voir avant d'agir. Lecture seule.
#[tauri::command(async)]
fn list_location_files(path: String, files: Vec<String>) -> CmdResult<FileList> {
    const MAX: usize = 1000;
    let root = PathBuf::from(&path);
    let modified = |m: &fs::Metadata| {
        m.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
    };
    let mut out = Vec::new();
    let mut truncated = false;
    if files.is_empty() {
        for e in walkdir::WalkDir::new(&root).follow_links(false).into_iter().flatten() {
            if !e.file_type().is_file() {
                continue;
            }
            if out.len() >= MAX {
                truncated = true;
                break;
            }
            let Ok(meta) = e.metadata() else { continue };
            let rel = e.path().strip_prefix(&root).unwrap_or(e.path()).to_string_lossy().to_string();
            out.push(FileEntry { path: rel, size: meta.len(), modified: modified(&meta) });
        }
    } else {
        for name in files.iter().take(MAX) {
            if let Ok(meta) = fs::metadata(root.join(name)) {
                out.push(FileEntry { path: name.clone(), size: meta.len(), modified: modified(&meta) });
            }
        }
        truncated = files.len() > MAX;
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(FileList { files: out, truncated })
}

#[tauri::command(async)]
fn is_steam_running() -> CmdResult<bool> {
    Ok(process::is_named_running("steam.exe"))
}

#[tauri::command(async)]
fn open_log_dir(app: AppHandle) -> CmdResult<()> {
    let dir = app.path().app_log_dir().map_err(err)?;
    fs::create_dir_all(&dir).map_err(err)?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(err)
}

#[tauri::command]
async fn list_backups(state: State<'_, AppState>) -> CmdResult<Vec<BackupInfo>> {
    let Ok(dir) = state.backup_dir() else { return Ok(vec![]) };
    tauri::async_runtime::spawn_blocking(move || backup::list(&dir)).await.map_err(err)
}

#[tauri::command]
async fn restore_backup(app: AppHandle, state: State<'_, AppState>, path: String) -> CmdResult<()> {
    let dir = state.backup_dir()?;
    let zip = PathBuf::from(&path);
    let meta = backup::read_meta(&zip).map_err(err)?;
    state.ensure_not_running(&meta.game)?;
    tauri::async_runtime::spawn_blocking(move || {
        let reporter = Reporter::new(&app, "restore", meta.game.clone());
        reporter.send(0, 0, false);
        backup::restore(&dir, &zip)?;
        log::info!("backup restauré : {}", zip.display());
        anyhow::Ok(())
    })
    .await
    .map_err(err)?
    .map_err(err)
}

#[tauri::command(async)]
fn delete_backup(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    let dir = state.backup_dir()?;
    let inside = paths::is_within(&paths::norm(Path::new(&path)), &paths::norm(&dir));
    if !inside || !path.to_lowercase().ends_with(".zip") {
        return Err("not_a_backup".into());
    }
    trash::delete(&path).map_err(err)
}

#[tauri::command]
async fn delete_game_saves(state: State<'_, AppState>, game_id: String) -> CmdResult<()> {
    let game = state.game(&game_id)?;
    let dir = state.backup_dir()?;
    state.ensure_not_running(&game_id)?;
    let locs: Vec<MetaLocation> = game.locations.iter().map(MetaLocation::from).collect();
    tauri::async_runtime::spawn_blocking(move || {
        if backup::has_files(&locs) {
            backup::create(&dir, &game.title, &locs, BackupKind::PreDelete, &|_, _| {})?;
        }
        backup::trash_locations(&locs)
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    let updated = state.scan.lock().unwrap().clone().map(|mut s| {
        s.games.retain(|g| g.id != game_id);
        s
    });
    if let Some(s) = updated {
        state.store_scan(s);
    }
    Ok(())
}

/// Un dossier non identifié, tel que listé par le dernier scan (jamais un chemin arbitraire).
fn unknown_dir(state: &AppState, path: &str) -> CmdResult<scan::UnknownDir> {
    state
        .scan
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|s| s.unidentified.iter().find(|u| u.path == path).cloned())
        .ok_or_else(|| "game_not_found".to_string())
}

fn unknown_locations(u: &scan::UnknownDir) -> Vec<MetaLocation> {
    vec![MetaLocation { path: u.path.clone(), kind: scan::LocationKind::Dir, files: vec![] }]
}

#[tauri::command]
async fn backup_unknown(state: State<'_, AppState>, path: String) -> CmdResult<BackupInfo> {
    let u = unknown_dir(&state, &path)?;
    let dir = state.backup_dir()?;
    tauri::async_runtime::spawn_blocking(move || backup::create(&dir, &u.name, &unknown_locations(&u), BackupKind::Manual, &|_, _| {}))
        .await
        .map_err(err)?
        .map_err(err)
}

#[tauri::command]
async fn delete_unknown(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    let u = unknown_dir(&state, &path)?;
    let dir = state.backup_dir()?;
    tauri::async_runtime::spawn_blocking(move || {
        let locs = unknown_locations(&u);
        if backup::has_files(&locs) {
            backup::create(&dir, &u.name, &locs, BackupKind::PreDelete, &|_, _| {})?;
        }
        backup::trash_locations(&locs)
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    let updated = state.scan.lock().unwrap().clone().map(|mut s| {
        s.unidentified.retain(|u| u.path != path);
        s
    });
    if let Some(s) = updated {
        state.store_scan(s);
    }
    Ok(())
}

#[tauri::command(async)]
fn is_game_running(state: State<'_, AppState>, game_id: String) -> CmdResult<Option<bool>> {
    let Some(dir) = state.game(&game_id).ok().and_then(|g| g.install_dir) else { return Ok(None) };
    Ok(Some(process::is_running_from(Path::new(&dir))))
}

#[tauri::command(async)]
fn open_path(app: tauri::AppHandle, path: String) -> CmdResult<()> {
    let p = Path::new(&path);
    if p.is_file() {
        app.opener().reveal_item_in_dir(p).map_err(err)
    } else {
        app.opener().open_path(path, None::<&str>).map_err(err)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CoverData {
    /// Image encodée en `data:` URL, affichable directement par la webview.
    src: String,
    portrait: bool,
}

#[tauri::command]
async fn get_cover(state: State<'_, AppState>, app_id: Option<u32>, title: String) -> CmdResult<Option<CoverData>> {
    use base64::Engine;
    let key = state.config().steamgriddb_key;
    let steam_root = stores::Stores::steam_root();
    let Some(cover) = covers::get(&state.covers_dir(), steam_root.as_deref(), app_id, &title, &key).await else {
        return Ok(None);
    };
    let original = PathBuf::from(&cover.path);
    let (path, bytes) = tauri::async_runtime::spawn_blocking(move || {
        let path = covers::thumbnail(&original);
        fs::read(&path).map(|b| (path, b))
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    let mime = match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(Some(CoverData { src: format!("data:{mime};base64,{encoded}"), portrait: cover.portrait }))
}

#[tauri::command(async)]
fn cover_cache_stats(state: State<'_, AppState>) -> CmdResult<covers::CacheStats> {
    Ok(covers::stats(&state.covers_dir()))
}

#[tauri::command(async)]
fn clear_cover_cache(state: State<'_, AppState>) -> CmdResult<()> {
    covers::clear(&state.covers_dir()).map_err(err)
}

#[tauri::command(async)]
fn search_titles(state: State<'_, AppState>, query: String) -> CmdResult<Vec<TitleHit>> {
    let q = query.trim().to_lowercase();
    if q.len() < 2 {
        return Ok(vec![]);
    }
    let guard = state.manifest.read().unwrap();
    let Some(m) = guard.as_ref() else { return Ok(vec![]) };
    let mut hits: Vec<&manifest::MGame> = m.games.iter().filter(|g| g.name.to_lowercase().contains(&q)).collect();
    hits.sort_by_key(|g| (!g.name.to_lowercase().starts_with(&q), g.name.len()));
    Ok(hits
        .into_iter()
        .take(20)
        .map(|g| TitleHit { name: g.name.clone(), steam_id: g.steam_id })
        .collect())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri_plugin_log::{RotationStrategy, Target, TargetKind};
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: Some("savevault".into()) }),
                ])
                .level(log::LevelFilter::Info)
                .max_file_size(2_000_000)
                .rotation_strategy(RotationStrategy::KeepOne)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let paths = app.path();
            let config_path = paths.app_config_dir()?.join("config.json");
            let data_dir = paths.app_data_dir()?;
            let cache_dir = paths.app_cache_dir()?;
            fs::create_dir_all(&data_dir)?;
            fs::create_dir_all(&cache_dir)?;
            app.manage(AppState {
                config: Mutex::new(AppConfig::load(&config_path)),
                config_path,
                manifest_files: Arc::new(ManifestFiles::new(&data_dir)),
                cache_dir,
                manifest: RwLock::new(None),
                scan: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            get_manifest_status,
            update_manifest,
            get_cached_scan,
            run_scan,
            create_backup,
            backup_games,
            list_location_files,
            is_steam_running,
            open_log_dir,
            list_backups,
            restore_backup,
            delete_backup,
            delete_game_saves,
            backup_unknown,
            delete_unknown,
            is_game_running,
            open_path,
            get_cover,
            cover_cache_stats,
            clear_cover_cache,
            search_titles,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
