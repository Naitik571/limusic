//! Artist Packs — per-artist ZIPs `artist.json + style.css` from R2
//! `artist-packs.sfg545.dev/v1/index.json` via reqwest + zip.
//! Stored under `app_data/artist_packs/<id>/`, DB table `artist_packs`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

const INDEX_URL: &str = "https://artist-packs.sfg545.dev/v1/index.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistPackIndexEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub artist_ids: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub url: String, // ZIP URL
    #[serde(default)]
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistPackIndex {
    pub packs: Vec<ArtistPackIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistJson {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub layout: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub artist_ids: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistPack {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub artist_ids: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub layout: Option<String>,
    pub style_css: Option<String>,
    pub installed_at: i64,
    #[serde(default)]
    pub thumbnail: Option<String>,
}

fn packs_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("artist_packs")
}

pub fn packs_dir_for(data_dir: &Path, id: &str) -> PathBuf {
    let safe: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    packs_dir(data_dir).join(safe)
}

/// List installed packs from DB.
pub fn list_packs(db: &crate::db::Db) -> Vec<ArtistPack> {
    let conn = db.conn_lock();
    let mut out = Vec::new();
    if let Ok(mut stmt) = conn.prepare("SELECT id, name, version, description, artist_ids, aliases, layout, style_css, installed_at, thumbnail FROM artist_packs ORDER BY installed_at DESC") {
        if let Ok(rows) = stmt.query_map([], |r| {
            let artist_ids_json: String = r.get(4)?;
            let aliases_json: String = r.get(5)?;
            Ok(ArtistPack {
                id: r.get(0)?,
                name: r.get(1)?,
                version: r.get(2)?,
                description: r.get(3)?,
                artist_ids: serde_json::from_str(&artist_ids_json).unwrap_or_default(),
                aliases: serde_json::from_str(&aliases_json).unwrap_or_default(),
                layout: r.get(6)?,
                style_css: r.get(7)?,
                installed_at: r.get(8)?,
                thumbnail: r.get(9)?,
            })
        }) {
            out.extend(rows.flatten());
        }
    }
    out
}

pub fn get_pack(db: &crate::db::Db, id: &str) -> Option<ArtistPack> {
    let conn = db.conn_lock();
    conn.query_row("SELECT id, name, version, description, artist_ids, aliases, layout, style_css, installed_at, thumbnail FROM artist_packs WHERE id = ?1", [id], |r| {
        let artist_ids_json: String = r.get(4)?;
        let aliases_json: String = r.get(5)?;
        Ok(ArtistPack {
            id: r.get(0)?,
            name: r.get(1)?,
            version: r.get(2)?,
            description: r.get(3)?,
            artist_ids: serde_json::from_str(&artist_ids_json).unwrap_or_default(),
            aliases: serde_json::from_str(&aliases_json).unwrap_or_default(),
            layout: r.get(6)?,
            style_css: r.get(7)?,
            installed_at: r.get(8)?,
            thumbnail: r.get(9)?,
        })
    }).ok()
}

pub fn remove_pack(db: &crate::db::Db, data_dir: &Path, id: &str) -> Result<(), String> {
    let conn = db.conn_lock();
    conn.execute("DELETE FROM artist_packs WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    drop(conn);
    let dir = packs_dir_for(data_dir, id);
    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

/// Install from a ZIP file on disk (already downloaded or user picked).
pub fn install_from_zip(
    db: &crate::db::Db,
    data_dir: &Path,
    zip_path: &Path,
) -> Result<ArtistPack, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut artist_json: Option<ArtistJson> = None;
    let mut style_css: Option<String> = None;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().to_string();
        if name.ends_with("artist.json") {
            let mut s = String::new();
            use std::io::Read;
            f.read_to_string(&mut s).map_err(|e| e.to_string())?;
            if let Ok(v) = serde_json::from_str::<ArtistJson>(&s) {
                artist_json = Some(v);
            }
        } else if name.ends_with("style.css") {
            let mut s = String::new();
            use std::io::Read;
            f.read_to_string(&mut s).map_err(|e| e.to_string())?;
            style_css = Some(s);
        }
    }
    let meta = artist_json.ok_or("ZIP missing artist.json")?;
    let id = meta.id.clone();
    let dir = packs_dir_for(data_dir, &id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // Re-extract files to disk for asset serving (style.css as file)
    // Re-open archive to extract all files.
    let file2 = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive2 = zip::ZipArchive::new(file2).map_err(|e| e.to_string())?;
    for i in 0..archive2.len() {
        let mut f = archive2.by_index(i).map_err(|e| e.to_string())?;
        let out_path = dir.join(f.name());
        if f.is_dir() {
            std::fs::create_dir_all(&out_path).ok();
        } else {
            if let Some(p) = out_path.parent() {
                std::fs::create_dir_all(p).ok();
            }
            let mut out = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
            use std::io::copy;
            copy(&mut f, &mut out).map_err(|e| e.to_string())?;
        }
    }
    // Persist style.css as data URI already? Store raw CSS string.
    let css_on_disk = style_css.clone().or_else(|| {
        let p = dir.join("style.css");
        std::fs::read_to_string(p).ok()
    });
    let pack = ArtistPack {
        id: meta.id.clone(),
        name: meta.name.clone(),
        version: meta.version.clone(),
        description: meta.description.clone(),
        artist_ids: meta.artist_ids.clone(),
        aliases: meta.aliases.clone(),
        layout: meta.layout.clone(),
        style_css: css_on_disk.clone(),
        installed_at: crate::db::now_secs(),
        thumbnail: None,
    };
    let conn = db.conn_lock();
    conn.execute(
        "INSERT INTO artist_packs(id, name, version, description, artist_ids, aliases, layout, style_css, installed_at, thumbnail)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
         ON CONFLICT(id) DO UPDATE SET name=excluded.name, version=excluded.version, description=excluded.description,
         artist_ids=excluded.artist_ids, aliases=excluded.aliases, layout=excluded.layout, style_css=excluded.style_css,
         installed_at=excluded.installed_at, thumbnail=excluded.thumbnail",
        rusqlite::params![
            pack.id, pack.name, pack.version, pack.description,
            serde_json::to_string(&pack.artist_ids).unwrap_or("[]".into()),
            serde_json::to_string(&pack.aliases).unwrap_or("[]".into()),
            pack.layout, pack.style_css, pack.installed_at, pack.thumbnail
        ],
    ).map_err(|e| e.to_string())?;
    Ok(pack)
}

/// Fetch index.json (15min poll caller).
pub async fn fetch_index() -> Result<ArtistPackIndex, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(INDEX_URL)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("index fetch {}", resp.status()));
    }
    let idx: ArtistPackIndex = resp.json().await.map_err(|e| e.to_string())?;
    Ok(idx)
}

/// Poll loop: every 15min fetch index and emit event if new packs available.
pub fn spawn_index_poller(app: tauri::AppHandle, _db: Arc<crate::db::Db>) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(15 * 60)).await;
            match fetch_index().await {
                Ok(idx) => {
                    let _ = app.emit("artist-packs-index", &idx);
                }
                Err(e) => {
                    tracing::debug!(error=%e, "artist packs index poll failed");
                }
            }
        }
    });
}

/// Download ZIP from URL and install.
pub async fn install_from_url(
    db: Arc<crate::db::Db>,
    data_dir: PathBuf,
    url: String,
) -> Result<ArtistPack, String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("only http(s) URLs".into());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let bytes = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    // zip crate needs Seek, write to temp file
    let tmp = data_dir.join(format!("pack-{}.zip", crate::db::now_secs()));
    std::fs::create_dir_all(&data_dir).ok();
    std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    let pack = install_from_zip(&db, &data_dir, &tmp)?;
    let _ = std::fs::remove_file(tmp);
    Ok(pack)
}
