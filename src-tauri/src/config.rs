//! Réglages de l'utilisateur, stockés en JSON dans le dossier de config de l'app.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomEntry {
    /// Nom du jeu auquel le dossier est rattaché.
    pub game: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub backup_dir: Option<String>,
    pub retention_enabled: bool,
    pub retention_count: u32,
    pub language: String,
    pub steamgriddb_key: String,
    pub check_updates_on_start: bool,
    pub custom_entries: Vec<CustomEntry>,
    pub ignored_paths: Vec<String>,
    /// Dossiers dont chaque sous-dossier est un jeu installé hors launcher (ex. `C:\Games`).
    pub game_dirs: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            backup_dir: None,
            retention_enabled: false,
            retention_count: 5,
            language: "fr".into(),
            steamgriddb_key: String::new(),
            check_updates_on_start: true,
            custom_entries: Vec::new(),
            ignored_paths: Vec::new(),
            game_dirs: vec![r"C:\Games".to_string()],
        }
    }
}

impl AppConfig {
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        fs::rename(tmp, path)?;
        Ok(())
    }
}
