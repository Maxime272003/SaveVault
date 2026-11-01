//! Saves stockées dans le registre (HKCU uniquement) : détection, export, import, suppression via `reg.exe`.

use anyhow::{Result, bail};
use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

#[cfg(windows)]
fn open(key: &str) -> Option<winreg::RegKey> {
    let (hive, rest) = key.split_once('\\')?;
    if !matches!(hive.to_uppercase().as_str(), "HKEY_CURRENT_USER" | "HKCU") {
        return None;
    }
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(rest).ok()
}

/// Nombre de valeurs + sous-clés si la clé existe.
pub fn entry_count(key: &str) -> Option<u64> {
    #[cfg(windows)]
    {
        let info = open(key)?.query_info().ok()?;
        Some(u64::from(info.values) + u64::from(info.sub_keys))
    }
    #[cfg(not(windows))]
    {
        let _ = key;
        None
    }
}

fn reg<I: IntoIterator<Item = S>, S: AsRef<OsStr>>(args: I) -> Result<()> {
    let mut cmd = Command::new("reg");
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output()?;
    if !out.status.success() {
        bail!("reg.exe : {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

pub fn export(key: &str, file: &Path) -> Result<()> {
    reg([OsStr::new("export"), OsStr::new(key), file.as_os_str(), OsStr::new("/y")])
}

pub fn import(file: &Path) -> Result<()> {
    reg([OsStr::new("import"), file.as_os_str()])
}

pub fn delete(key: &str) -> Result<()> {
    if entry_count(key).is_none() {
        return Ok(());
    }
    reg(["delete", key, "/f"])
}
