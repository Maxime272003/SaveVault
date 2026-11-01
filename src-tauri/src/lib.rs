mod backup;
mod config;
mod covers;
mod manifest;
mod paths;
mod process;
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
use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
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

#[tauri::command]
async fn update_manifest(state: State<'_, AppState>, force: bool) -> CmdResult<ManifestStatus> {
    if let Some((m, _)) = manifest::download(&state.manifest_files, force).await.map_err(err)? {
        *state.manifest.write().unwrap() = Some(Arc::new(m));
    }
    Ok(manifest_status(&state))
}

async fn ensure_manifest(state: &AppState) -> CmdResult<Arc<Manifest>> {
    if let Some(m) = state.manifest.read().unwrap().clone() {
        return Ok(m);
    }
    let files = state.manifest_files.clone();
    let loaded = tauri::async_runtime::spawn_blocking(move || files.load()).await.map_err(err)?;
    let m = match loaded {
        Some(m) => m,
        None => manifest::download(&state.manifest_files, true)
            .await
            .map_err(err)?
            .map(|(m, _)| m)
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
async fn run_scan(state: State<'_, AppState>) -> CmdResult<ScanResult> {
    let manifest = ensure_manifest(&state).await?;
    let config = state.config();
    let result = tauri::async_runtime::spawn_blocking(move || scan::run(&manifest, &config))
        .await
        .map_err(err)?;
    state.store_scan(result.clone());
    Ok(result)
}

#[tauri::command]
async fn create_backup(state: State<'_, AppState>, game_id: String) -> CmdResult<BackupInfo> {
    let game = state.game(&game_id)?;
    let dir = state.backup_dir()?;
    let config = state.config();
    tauri::async_runtime::spawn_blocking(move || {
        let locs: Vec<MetaLocation> = game.locations.iter().map(MetaLocation::from).collect();
        let info = backup::create(&dir, &game.title, &locs, BackupKind::Manual)?;
        if config.retention_enabled {
            backup::apply_retention(&dir, &game.title, config.retention_count)?;
        }
        anyhow::Ok(info)
    })
    .await
    .map_err(err)?
    .map_err(err)
}

#[tauri::command]
async fn list_backups(state: State<'_, AppState>) -> CmdResult<Vec<BackupInfo>> {
    let Ok(dir) = state.backup_dir() else { return Ok(vec![]) };
    tauri::async_runtime::spawn_blocking(move || backup::list(&dir)).await.map_err(err)
}

#[tauri::command]
async fn restore_backup(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    let dir = state.backup_dir()?;
    let zip = PathBuf::from(&path);
    let meta = backup::read_meta(&zip).map_err(err)?;
    state.ensure_not_running(&meta.game)?;
    tauri::async_runtime::spawn_blocking(move || backup::restore(&dir, &zip))
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
            backup::create(&dir, &game.title, &locs, BackupKind::PreDelete)?;
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
    tauri::async_runtime::spawn_blocking(move || backup::create(&dir, &u.name, &unknown_locations(&u), BackupKind::Manual))
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
            backup::create(&dir, &u.name, &locs, BackupKind::PreDelete)?;
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
    let path = PathBuf::from(&cover.path);
    let mime = match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    };
    let bytes = tauri::async_runtime::spawn_blocking(move || fs::read(path)).await.map_err(err)?.map_err(err)?;
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
    tauri::Builder::default()
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
