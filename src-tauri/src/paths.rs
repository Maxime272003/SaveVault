//! Dossiers Windows connus et petites aides sur les chemins.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct KnownDirs {
    pub home: Option<PathBuf>,
    pub app_data: Option<PathBuf>,
    pub local_app_data: Option<PathBuf>,
    pub local_low: Option<PathBuf>,
    pub documents: Option<PathBuf>,
    pub saved_games: Option<PathBuf>,
    pub public: Option<PathBuf>,
    pub program_data: Option<PathBuf>,
    pub win_dir: Option<PathBuf>,
    pub user_name: Option<String>,
}

impl KnownDirs {
    pub fn detect() -> Self {
        let home = dirs::home_dir();
        let env_dir = |name: &str| std::env::var_os(name).map(PathBuf::from);
        Self {
            local_low: home.as_ref().map(|h| h.join("AppData").join("LocalLow")),
            saved_games: home.as_ref().map(|h| h.join("Saved Games")),
            app_data: dirs::config_dir(),
            local_app_data: dirs::data_local_dir(),
            documents: dirs::document_dir(),
            public: env_dir("PUBLIC"),
            program_data: env_dir("ProgramData"),
            win_dir: env_dir("WINDIR"),
            user_name: std::env::var("USERNAME").ok(),
            home,
        }
    }

    /// Dossiers qu'on ne doit jamais considérer comme « une save » à supprimer.
    pub fn protected(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = [
            &self.home,
            &self.app_data,
            &self.local_app_data,
            &self.local_low,
            &self.documents,
            &self.saved_games,
            &self.public,
            &self.program_data,
            &self.win_dir,
        ]
        .into_iter()
        .flatten()
        .cloned()
        .collect();
        if let Some(home) = &self.home {
            out.push(home.join("AppData"));
        }
        if let Some(docs) = &self.documents {
            out.push(docs.join("My Games"));
        }
        out
    }
}

/// Clé de comparaison : minuscules, séparateurs `\`, sans séparateur final.
pub fn norm(path: &Path) -> String {
    let mut s = path.to_string_lossy().replace('/', "\\").to_lowercase();
    while s.ends_with('\\') && s.len() > 3 {
        s.pop();
    }
    s
}

/// `child` est-il `parent` ou un de ses descendants ?
pub fn is_within(child: &str, parent: &str) -> bool {
    child == parent || (child.starts_with(parent) && child[parent.len()..].starts_with('\\'))
}

/// Nom de fichier sans caractères interdits par Windows.
pub fn sanitize_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches('.').trim();
    if trimmed.is_empty() {
        "jeu".to_string()
    } else {
        trimmed.chars().take(80).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn within() {
        assert!(is_within(r"c:\a\b", r"c:\a"));
        assert!(is_within(r"c:\a", r"c:\a"));
        assert!(!is_within(r"c:\ab", r"c:\a"));
    }

    #[test]
    fn sanitize() {
        assert_eq!(sanitize_file_name("Baldur's Gate 3: Deluxe?"), "Baldur's Gate 3_ Deluxe_");
    }
}
