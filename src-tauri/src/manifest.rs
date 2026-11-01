//! Manifeste Ludusavi : téléchargé depuis GitHub, réduit à ce qui sert sous Windows,
//! puis mis en cache en JSON pour des démarrages rapides.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/mtkennerly/ludusavi-manifest/master/data/manifest.yaml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub games: Vec<MGame>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MGame {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gog_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub install_dirs: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cloud_steam: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<MFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MFile {
    pub path: String,
    /// Stores pour lesquels ce chemin s'applique ; vide = tous.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stores: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestMeta {
    pub etag: Option<String>,
    pub updated_at: Option<i64>,
    pub game_count: usize,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawGame {
    files: BTreeMap<String, RawFile>,
    #[serde(rename = "installDir")]
    install_dir: BTreeMap<String, serde_yaml::Value>,
    steam: Option<RawId>,
    gog: Option<RawId>,
    cloud: Option<RawCloud>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawId {
    id: Option<u64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawCloud {
    steam: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawFile {
    tags: Option<Vec<String>>,
    when: Option<Vec<RawWhen>>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawWhen {
    os: Option<String>,
    store: Option<String>,
}

/// Convertit le YAML complet de Ludusavi en manifeste réduit.
pub fn from_yaml(yaml: &str) -> Result<Manifest> {
    let raw: BTreeMap<String, RawGame> =
        serde_yaml::from_str(yaml).context("manifeste Ludusavi illisible")?;
    let games = raw
        .into_iter()
        .map(|(name, g)| {
            let files = g
                .files
                .into_iter()
                .filter_map(|(path, f)| windows_save_file(path, f))
                .collect();
            MGame {
                name,
                steam_id: g.steam.and_then(|s| s.id).and_then(|id| u32::try_from(id).ok()),
                gog_id: g.gog.and_then(|s| s.id),
                install_dirs: g.install_dir.into_keys().collect(),
                cloud_steam: g.cloud.map(|c| c.steam).unwrap_or(false),
                files,
            }
        })
        .collect();
    Ok(Manifest { games })
}

fn windows_save_file(path: String, f: RawFile) -> Option<MFile> {
    if let Some(tags) = &f.tags {
        if !tags.iter().any(|t| t == "save") {
            return None;
        }
    }
    let conditions = f.when.unwrap_or_default();
    if conditions.is_empty() {
        return Some(MFile { path, stores: vec![] });
    }
    let windows: Vec<&RawWhen> = conditions
        .iter()
        .filter(|w| w.os.as_deref().is_none_or(|os| os == "windows"))
        .collect();
    if windows.is_empty() {
        return None;
    }
    let stores = if windows.iter().any(|w| w.store.is_none()) {
        vec![]
    } else {
        let mut s: Vec<String> = windows.iter().filter_map(|w| w.store.clone()).collect();
        s.sort();
        s.dedup();
        s
    };
    Some(MFile { path, stores })
}

pub struct ManifestFiles {
    pub data: PathBuf,
    pub meta: PathBuf,
}

impl ManifestFiles {
    pub fn new(dir: &Path) -> Self {
        Self {
            data: dir.join("manifest.json"),
            meta: dir.join("manifest-meta.json"),
        }
    }

    pub fn load(&self) -> Option<Manifest> {
        let bytes = fs::read(&self.data).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn load_meta(&self) -> ManifestMeta {
        fs::read(&self.meta)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn save(&self, manifest: &Manifest, meta: &ManifestMeta) -> Result<()> {
        if let Some(parent) = self.data.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.data, serde_json::to_vec(manifest)?)?;
        fs::write(&self.meta, serde_json::to_vec_pretty(meta)?)?;
        Ok(())
    }
}

/// Télécharge le manifeste si il a changé. Renvoie `None` si la copie locale est à jour.
pub async fn download(files: &ManifestFiles, force: bool) -> Result<Option<(Manifest, ManifestMeta)>> {
    let previous = files.load_meta();
    let client = reqwest::Client::builder()
        .user_agent(concat!("SaveVault/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let mut req = client.get(MANIFEST_URL);
    if !force && files.data.exists() {
        if let Some(etag) = &previous.etag {
            req = req.header(reqwest::header::IF_NONE_MATCH, etag);
        }
    }
    let resp = req.send().await.context("téléchargement du manifeste impossible")?;
    let now = chrono::Utc::now().timestamp_millis();
    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        let meta = ManifestMeta { updated_at: Some(now), ..previous };
        fs::write(&files.meta, serde_json::to_vec_pretty(&meta)?)?;
        return Ok(None);
    }
    let resp = resp.error_for_status()?;
    let etag = resp
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let text = resp.text().await?;
    let manifest = tauri::async_runtime::spawn_blocking(move || from_yaml(&text)).await??;
    let meta = ManifestMeta {
        etag,
        updated_at: Some(now),
        game_count: manifest.games.len(),
    };
    files.save(&manifest, &meta)?;
    Ok(Some((manifest, meta)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_windows_saves() {
        let yaml = r#"
Hollow Knight:
  cloud:
    steam: true
  files:
    "<base>/hollow_knight_Data/Config.ini":
      tags: [config]
      when: [{os: windows}]
    "<home>/AppData/LocalLow/Team Cherry/Hollow Knight/*.dat":
      tags: [save]
      when: [{os: windows}]
    "<home>/Library/Application Support/unity.Team Cherry.Hollow Knight":
      tags: [save]
      when: [{os: mac}]
    "<root>/userdata/<storeUserId>/367520/remote":
      when: [{store: steam}]
  installDir:
    Hollow Knight: {}
  steam:
    id: 367520
"#;
        let m = from_yaml(yaml).unwrap();
        let g = &m.games[0];
        assert_eq!(g.steam_id, Some(367520));
        assert!(g.cloud_steam);
        assert_eq!(g.install_dirs, vec!["Hollow Knight"]);
        assert_eq!(g.files.len(), 2);
        assert_eq!(g.files[1].stores, vec!["steam"]);
    }
}
