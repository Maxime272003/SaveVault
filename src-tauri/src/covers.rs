//! Jaquettes : cache local de Steam, CDN Steam, puis SteamGridDB (clé optionnelle).
//! Tout est copié dans le dossier de cache de l'app pour fonctionner hors ligne.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cover {
    pub path: String,
    /// `false` quand on n'a trouvé qu'une bannière horizontale.
    pub portrait: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStats {
    pub count: u64,
    pub bytes: u64,
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("SaveVault/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("client HTTP")
    })
}

fn fnv(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.to_lowercase().bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn found(path: PathBuf, portrait: bool) -> Option<Cover> {
    Some(Cover { path: path.to_string_lossy().to_string(), portrait })
}

pub async fn get(
    cache: &Path,
    steam_root: Option<&Path>,
    app_id: Option<u32>,
    title: &str,
    sgdb_key: &str,
) -> Option<Cover> {
    fs::create_dir_all(cache).ok()?;
    if let Some(id) = app_id {
        let portrait = cache.join(format!("steam_{id}.jpg"));
        let landscape = cache.join(format!("steam_{id}_h.jpg"));
        let none = cache.join(format!("steam_{id}.none"));
        if portrait.exists() {
            return found(portrait, true);
        }
        if landscape.exists() {
            return found(landscape, false);
        }
        if !none.exists() {
            if let Some(local) = steam_root.and_then(|r| local_steam_cover(r, id)) {
                if fs::copy(&local, &portrait).is_ok() {
                    return found(portrait, true);
                }
            }
            let portrait_urls = [
                format!("https://shared.cloudflare.steamstatic.com/store_item_assets/steam/apps/{id}/library_600x900.jpg"),
                format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{id}/library_600x900.jpg"),
            ];
            if download_first(&portrait_urls, &portrait).await {
                return found(portrait, true);
            }
            let header_urls = [
                format!("https://shared.cloudflare.steamstatic.com/store_item_assets/steam/apps/{id}/header.jpg"),
                format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{id}/header.jpg"),
            ];
            if download_first(&header_urls, &landscape).await {
                return found(landscape, false);
            }
            let _ = fs::write(&none, b"");
        }
    }

    let key = sgdb_key.trim();
    if key.is_empty() || title.is_empty() {
        return None;
    }
    let h = fnv(title);
    const EXTS: [&str; 3] = ["png", "jpg", "webp"];
    if let Some(cached) = EXTS.iter().map(|x| cache.join(format!("sgdb_{h}.{x}"))).find(|p| p.exists()) {
        return found(cached, true);
    }
    let none = cache.join(format!("sgdb_{h}.none"));
    if none.exists() {
        return None;
    }
    if let Some(url) = steamgriddb(key, title).await {
        let ext = EXTS.iter().find(|x| url.to_lowercase().ends_with(*x)).unwrap_or(&"jpg");
        let target = cache.join(format!("sgdb_{h}.{ext}"));
        if download_first(&[url], &target).await {
            return found(target, true);
        }
    }
    let _ = fs::write(&none, b"");
    None
}

/// Steam garde des jaquettes dans `appcache\librarycache`, à plat ou dans un sous-dossier par jeu.
fn local_steam_cover(root: &Path, id: u32) -> Option<PathBuf> {
    let cache = root.join("appcache").join("librarycache");
    let flat = cache.join(format!("{id}_library_600x900.jpg"));
    if flat.is_file() {
        return Some(flat);
    }
    walkdir::WalkDir::new(cache.join(id.to_string()))
        .max_depth(3)
        .into_iter()
        .flatten()
        .find(|e| {
            let n = e.file_name().to_string_lossy().to_lowercase();
            e.file_type().is_file() && n.starts_with("library_600x900") && n.ends_with(".jpg")
        })
        .map(|e| e.into_path())
}

async fn download_first(urls: &[String], target: &Path) -> bool {
    for url in urls {
        let Ok(resp) = client().get(url).send().await else { continue };
        if !resp.status().is_success() {
            continue;
        }
        let is_image = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.starts_with("image/"));
        let Ok(bytes) = resp.bytes().await else { continue };
        if is_image && bytes.len() > 1024 && fs::write(target, &bytes).is_ok() {
            return true;
        }
    }
    false
}

#[derive(Deserialize)]
struct SgdbList<T> {
    #[serde(default = "Vec::new")]
    data: Vec<T>,
}

#[derive(Deserialize)]
struct SgdbGame {
    id: u64,
}

#[derive(Deserialize)]
struct SgdbGrid {
    url: String,
}

async fn steamgriddb(key: &str, title: &str) -> Option<String> {
    let base = "https://www.steamgriddb.com/api/v2";
    let search: SgdbList<SgdbGame> = client()
        .get(format!("{base}/search/autocomplete/{}", urlencoding::encode(title)))
        .bearer_auth(key)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;
    let game = search.data.first()?;
    let grids: SgdbList<SgdbGrid> = client()
        .get(format!("{base}/grids/game/{}?dimensions=600x900", game.id))
        .bearer_auth(key)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;
    grids.data.into_iter().next().map(|g| g.url)
}

pub fn stats(cache: &Path) -> CacheStats {
    let mut s = CacheStats { count: 0, bytes: 0 };
    if let Ok(entries) = fs::read_dir(cache) {
        for e in entries.flatten() {
            if e.path().extension().is_some_and(|x| x == "none") {
                continue;
            }
            if let Ok(m) = e.metadata() {
                s.count += 1;
                s.bytes += m.len();
            }
        }
    }
    s
}

pub fn clear(cache: &Path) -> std::io::Result<()> {
    if cache.exists() {
        fs::remove_dir_all(cache)?;
    }
    fs::create_dir_all(cache)
}
