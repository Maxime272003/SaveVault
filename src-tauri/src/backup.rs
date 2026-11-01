//! Backups zip, restauration et suppression (toujours vers la corbeille).

use crate::paths;
use crate::scan::{Location, LocationKind};
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Component, Path, PathBuf};
use zip::write::SimpleFileOptions;

const META_NAME: &str = "savevault.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackupKind {
    Manual,
    Auto,
    PreRestore,
    PreDelete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaLocation {
    pub path: String,
    pub kind: LocationKind,
    #[serde(default)]
    pub files: Vec<String>,
}

impl From<&Location> for MetaLocation {
    fn from(l: &Location) -> Self {
        Self { path: l.path.clone(), kind: l.kind, files: l.files.clone() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupMeta {
    pub format: u32,
    pub app_version: String,
    pub game: String,
    pub created_at: i64,
    pub kind: BackupKind,
    pub locations: Vec<MetaLocation>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: String,
    pub file_name: String,
    pub game: String,
    pub created_at: i64,
    pub size: u64,
    pub kind: BackupKind,
}

/// Fichiers à mettre dans le zip : (fichier source, nom dans l'archive).
fn collect_entries(locations: &[MetaLocation]) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    for (i, loc) in locations.iter().enumerate() {
        let root = Path::new(&loc.path);
        match loc.kind {
            LocationKind::Dir => {
                for e in walkdir::WalkDir::new(root).follow_links(false).into_iter().flatten() {
                    if !e.file_type().is_file() {
                        continue;
                    }
                    let Ok(rel) = e.path().strip_prefix(root) else { continue };
                    let rel = rel.to_string_lossy().replace('\\', "/");
                    out.push((e.path().to_path_buf(), format!("files/{i}/{rel}")));
                }
            }
            LocationKind::Files => {
                for name in &loc.files {
                    let p = root.join(name);
                    if p.is_file() {
                        out.push((p, format!("files/{i}/{name}")));
                    }
                }
            }
            // Exportées à part, au moment du backup (voir `export_registry`).
            LocationKind::Registry => {}
        }
    }
    out
}

const REGISTRY_FILE: &str = "registry.reg";

/// Dossier temporaire supprimé à la fin du backup.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Result<Self> {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
        let p = std::env::temp_dir().join(format!("savevault-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&p)?;
        Ok(Self(p))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Exporte les clés de registre existantes en `.reg` dans `tmp`.
fn export_registry(locations: &[MetaLocation], tmp: &TempDir) -> Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    for (i, loc) in locations.iter().enumerate() {
        if loc.kind == LocationKind::Registry && crate::registry::entry_count(&loc.path).is_some() {
            let file = tmp.0.join(format!("{i}.reg"));
            crate::registry::export(&loc.path, &file)?;
            out.push((file, format!("files/{i}/{REGISTRY_FILE}")));
        }
    }
    Ok(out)
}

pub fn has_files(locations: &[MetaLocation]) -> bool {
    !collect_entries(locations).is_empty()
        || locations
            .iter()
            .any(|l| l.kind == LocationKind::Registry && crate::registry::entry_count(&l.path).is_some())
}

fn game_dir(backup_dir: &Path, game: &str) -> PathBuf {
    backup_dir.join(paths::sanitize_file_name(game))
}

/// Relit toute l'archive : le contrôle CRC de chaque fichier détecte un zip corrompu.
fn verify(zip_path: &Path, expected_entries: usize) -> Result<()> {
    let mut archive = zip::ZipArchive::new(File::open(zip_path)?)?;
    if archive.len() != expected_entries {
        bail!("backup incomplet : {} fichiers au lieu de {expected_entries}", archive.len());
    }
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        io::copy(&mut entry, &mut io::sink()).with_context(|| format!("backup corrompu ({name})"))?;
    }
    Ok(())
}

/// `on_progress(octets traités, octets au total)`.
pub type Progress<'a> = &'a (dyn Fn(u64, u64) + Sync);

pub fn create(
    backup_dir: &Path,
    game: &str,
    locations: &[MetaLocation],
    kind: BackupKind,
    on_progress: Progress,
) -> Result<BackupInfo> {
    let tmp_dir = TempDir::new()?;
    let mut entries = collect_entries(locations);
    entries.extend(export_registry(locations, &tmp_dir)?);
    if entries.is_empty() {
        bail!("nothing_to_backup");
    }
    let total: u64 = entries.iter().filter_map(|(p, _)| fs::metadata(p).ok()).map(|m| m.len()).sum();
    let now = chrono::Local::now();
    let dir = game_dir(backup_dir, game);
    fs::create_dir_all(&dir).with_context(|| format!("impossible de créer {}", dir.display()))?;
    let suffix = match kind {
        BackupKind::Manual => "",
        BackupKind::Auto => "_auto",
        BackupKind::PreRestore => "_avant-restauration",
        BackupKind::PreDelete => "_avant-suppression",
    };
    let stem = format!("{}_{}{}", paths::sanitize_file_name(game), now.format("%Y-%m-%d_%H-%M-%S"), suffix);
    let mut target = dir.join(format!("{stem}.zip"));
    let mut n = 2;
    while target.exists() {
        target = dir.join(format!("{stem}-{n}.zip"));
        n += 1;
    }
    let meta = BackupMeta {
        format: 1,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        game: game.to_string(),
        created_at: now.timestamp_millis(),
        kind,
        locations: locations.to_vec(),
    };

    let tmp = target.with_extension("zip.part");
    let result = (|| -> Result<()> {
        let mut zip = zip::ZipWriter::new(BufWriter::new(File::create(&tmp)?));
        let opts = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        zip.start_file(META_NAME, opts)?;
        zip.write_all(&serde_json::to_vec_pretty(&meta)?)?;
        let mut done = 0;
        on_progress(0, total);
        for (src, name) in &entries {
            zip.start_file(name.as_str(), opts)?;
            let mut f = File::open(src).with_context(|| format!("lecture impossible : {}", src.display()))?;
            done += io::copy(&mut f, &mut zip)?;
            on_progress(done, total);
        }
        zip.finish()?.flush()?;
        verify(&tmp, entries.len() + 1)?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    fs::rename(&tmp, &target)?;
    info_for(&target, meta)
}

fn info_for(path: &Path, meta: BackupMeta) -> Result<BackupInfo> {
    Ok(BackupInfo {
        path: path.to_string_lossy().to_string(),
        file_name: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        game: meta.game,
        created_at: meta.created_at,
        size: fs::metadata(path)?.len(),
        kind: meta.kind,
    })
}

pub fn read_meta(path: &Path) -> Result<BackupMeta> {
    let mut archive = zip::ZipArchive::new(File::open(path)?)?;
    let entry = archive.by_name(META_NAME).context("ce zip n'est pas un backup SaveVault")?;
    Ok(serde_json::from_reader(entry)?)
}

pub fn list(backup_dir: &Path) -> Vec<BackupInfo> {
    let mut out: Vec<BackupInfo> = walkdir::WalkDir::new(backup_dir)
        .max_depth(2)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("zip")))
        .filter_map(|e| read_meta(e.path()).ok().and_then(|m| info_for(e.path(), m).ok()))
        .collect();
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    out
}

/// Supprime définitivement les plus anciens backups manuels et automatiques d'un jeu au-delà de `keep`.
pub fn apply_retention(backup_dir: &Path, game: &str, keep: u32) -> Result<()> {
    let mut backups: Vec<BackupInfo> = list(&game_dir(backup_dir, game))
        .into_iter()
        .filter(|b| b.game == game && matches!(b.kind, BackupKind::Manual | BackupKind::Auto))
        .collect();
    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    for old in backups.into_iter().skip(keep.max(1) as usize) {
        fs::remove_file(&old.path)?;
    }
    Ok(())
}

/// Envoie à la corbeille ce que couvrent ces emplacements.
pub fn trash_locations(locations: &[MetaLocation]) -> Result<()> {
    let mut targets: Vec<PathBuf> = Vec::new();
    for loc in locations {
        let root = PathBuf::from(&loc.path);
        match loc.kind {
            LocationKind::Dir if root.exists() => targets.push(root),
            LocationKind::Dir => {}
            LocationKind::Files => targets.extend(loc.files.iter().map(|f| root.join(f)).filter(|p| p.exists())),
            LocationKind::Registry => crate::registry::delete(&loc.path)?,
        }
    }
    if !targets.is_empty() {
        trash::delete_all(&targets).map_err(|e| anyhow!("envoi à la corbeille impossible : {e}"))?;
    }
    Ok(())
}

/// Restaure un backup : backup de l'état actuel, corbeille, puis extraction.
pub fn restore(backup_dir: &Path, zip_path: &Path) -> Result<()> {
    let meta = read_meta(zip_path)?;
    if has_files(&meta.locations) {
        create(backup_dir, &meta.game, &meta.locations, BackupKind::PreRestore, &|_, _| {})?;
    }
    trash_locations(&meta.locations)?;

    let mut archive = zip::ZipArchive::new(File::open(zip_path)?)?;
    for loc in &meta.locations {
        if loc.kind == LocationKind::Dir {
            fs::create_dir_all(&loc.path)?;
        }
    }
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry.enclosed_name() else { continue };
        let mut comps = name.components();
        if comps.next() != Some(Component::Normal("files".as_ref())) {
            continue;
        }
        let Some(idx) = comps
            .next()
            .and_then(|c| c.as_os_str().to_str())
            .and_then(|s| s.parse::<usize>().ok())
        else {
            continue;
        };
        let Some(loc) = meta.locations.get(idx) else { continue };
        let rel: PathBuf = comps.collect();
        if rel.as_os_str().is_empty() {
            continue;
        }
        if loc.kind == LocationKind::Registry {
            // La clé a déjà été supprimée par `trash_locations` : on réimporte l'export.
            let tmp = TempDir::new()?;
            let file = tmp.0.join(REGISTRY_FILE);
            io::copy(&mut entry, &mut File::create(&file)?)?;
            crate::registry::import(&file)?;
            continue;
        }
        let dest = Path::new(&loc.path).join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&dest).with_context(|| format!("écriture impossible : {}", dest.display()))?;
        io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("savevault-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn backup_roundtrip() {
        let root = tmp("roundtrip");
        let save = root.join("save");
        fs::create_dir_all(save.join("sub")).unwrap();
        fs::write(save.join("slot1.sav"), b"hello").unwrap();
        fs::write(save.join("sub").join("profile.dat"), b"world").unwrap();
        let backups = root.join("backups");
        let locs = vec![MetaLocation { path: save.to_string_lossy().into(), kind: LocationKind::Dir, files: vec![] }];

        let info = create(&backups, "Mon Jeu: test", &locs, BackupKind::Manual, &|_, _| {}).unwrap();
        assert!(info.file_name.starts_with("Mon Jeu_ test_"));
        let listed = list(&backups);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].game, "Mon Jeu: test");

        // Extraction manuelle (sans passer par la corbeille, indisponible en CI).
        fs::remove_dir_all(&save).unwrap();
        let meta = read_meta(Path::new(&info.path)).unwrap();
        assert_eq!(meta.locations.len(), 1);
        let mut archive = zip::ZipArchive::new(File::open(&info.path).unwrap()).unwrap();
        let names: Vec<String> = (0..archive.len()).map(|i| archive.by_index(i).unwrap().name().to_string()).collect();
        assert!(names.contains(&"files/0/slot1.sav".to_string()));
        assert!(names.contains(&"files/0/sub/profile.dat".to_string()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn verify_detects_corruption() {
        let root = tmp("verify");
        let save = root.join("s");
        fs::create_dir_all(&save).unwrap();
        // Contenu peu compressible pour que l'altération touche les données d'un fichier.
        let data: Vec<u8> = (0..200_000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
        fs::write(save.join("big.sav"), &data).unwrap();
        let locs = vec![MetaLocation { path: save.to_string_lossy().into(), kind: LocationKind::Dir, files: vec![] }];
        let info = create(&root.join("b"), "G", &locs, BackupKind::Manual, &|_, _| {}).unwrap();
        verify(Path::new(&info.path), 2).unwrap();

        let mut bytes = fs::read(&info.path).unwrap();
        let mid = bytes.len() / 2;
        for b in &mut bytes[mid..mid + 64] {
            *b ^= 0xff;
        }
        fs::write(&info.path, bytes).unwrap();
        assert!(verify(Path::new(&info.path), 2).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn retention_keeps_newest() {
        let root = tmp("retention");
        let save = root.join("s");
        fs::create_dir_all(&save).unwrap();
        fs::write(save.join("a.sav"), b"x").unwrap();
        let backups = root.join("b");
        let locs = vec![MetaLocation { path: save.to_string_lossy().into(), kind: LocationKind::Dir, files: vec![] }];
        for _ in 0..3 {
            create(&backups, "G", &locs, BackupKind::Manual, &|_, _| {}).unwrap();
        }
        apply_retention(&backups, "G", 2).unwrap();
        assert_eq!(list(&backups).len(), 2);
        let _ = fs::remove_dir_all(&root);
    }
}
