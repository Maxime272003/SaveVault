//! Détection d'un jeu en cours d'exécution (processus lancé depuis son dossier d'installation).

use crate::paths;
use std::path::Path;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// Un processus portant ce nom d'exécutable (ex. `steam.exe`) tourne-t-il ?
pub fn is_named_running(exe_name: &str) -> bool {
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    sys.processes()
        .values()
        .any(|p| p.name().to_string_lossy().eq_ignore_ascii_case(exe_name))
}

pub fn is_running_from(install_dir: &Path) -> bool {
    let dir = paths::norm(install_dir);
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
    );
    sys.processes()
        .values()
        .filter_map(|p| p.exe())
        .any(|exe| paths::is_within(&paths::norm(exe), &dir))
}
