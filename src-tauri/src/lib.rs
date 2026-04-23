use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex as TokioMutex;
use tokio::sync::Semaphore;
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

// ============================================================================
// Types
// ============================================================================

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub name: String,
    pub source: String,
    pub app_id: String,
    pub install_path: String,
    pub launch_url: String,
    pub size_bytes: u64,
    pub cover_url: Option<String>,
    pub hero_url: Option<String>,
    pub logo_url: Option<String>,
    pub cover_local: Option<String>,
    pub hero_local: Option<String>,
    pub logo_local: Option<String>,
    pub playtime_minutes: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct ScanProgress {
    stage: String,
    done: u32,
    total: u32,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct PlaytimeUpdate {
    game_id: String,
    playtime_minutes: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct Prefs {
    show_steam: bool,
    show_epic: bool,
    fullscreen: bool,
}

pub struct AppStateInner {
    pub db: Arc<Mutex<Connection>>,
    pub assets_dir: PathBuf,
    pub watchers: Arc<Mutex<HashSet<String>>>,
    pub http: reqwest::Client,
    pub scan_lock: Arc<TokioMutex<()>>,
}

// ============================================================================
// Helpers
// ============================================================================

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn safe_id(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn guess_ext(url: &str) -> &'static str {
    let lower = url.to_lowercase();
    if lower.contains(".png") {
        "png"
    } else if lower.contains(".gif") {
        "gif"
    } else if lower.contains(".webp") {
        "webp"
    } else {
        "jpg"
    }
}

fn steam_cdn(appid: &str, asset: &str) -> String {
    format!(
        "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/{}",
        appid, asset
    )
}

fn normalize_title(s: &str) -> String {
    let stripped: String = s
        .chars()
        .filter(|c| !matches!(*c, '\u{2122}' | '\u{00AE}' | '\u{00A9}'))
        .collect();
    stripped
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

// ============================================================================
// Steam utility filter
// ============================================================================

const EXCLUDED_STEAM_APPIDS: &[u64] = &[
    228980, 250820, 323910, 1070560, 1391110, 1628350, 1493710, 1887720, 1420170, 2348590,
    2180100,
];

fn is_excluded_steam_entry(appid: u64, name: &str) -> bool {
    if EXCLUDED_STEAM_APPIDS.contains(&appid) {
        return true;
    }
    let lower = name.to_lowercase();
    lower.contains("steamworks common")
        || lower.contains("redistributable")
        || lower.starts_with("proton ")
        || lower == "proton"
        || lower.starts_with("steam linux runtime")
}

// ============================================================================
// Steam install path + localconfig.vdf playtime (for one-time backfill)
// ============================================================================

fn steam_install_path() -> Option<PathBuf> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey("Software\\Valve\\Steam").ok()?;
    let path: String = key.get_value("SteamPath").ok()?;
    let pb = PathBuf::from(path);
    if pb.exists() {
        Some(pb)
    } else {
        None
    }
}

/// Per-appid lifetime minutes, as recorded by Steam itself. Only used for
/// the first-run backfill — after that, we record sessions natively.
fn load_steam_playtimes() -> HashMap<u64, u64> {
    let mut out: HashMap<u64, u64> = HashMap::new();
    let Some(steam) = steam_install_path() else {
        return out;
    };
    let userdata = steam.join("userdata");
    let Ok(entries) = fs::read_dir(&userdata) else {
        return out;
    };
    for entry in entries.flatten() {
        let config_path = entry.path().join("config").join("localconfig.vdf");
        if !config_path.is_file() {
            continue;
        }
        let Ok(content) = fs::read_to_string(&config_path) else {
            continue;
        };
        let Ok(vdf) = keyvalues_parser::Vdf::parse(&content) else {
            continue;
        };
        walk_for_playtimes(&vdf.value, &mut out);
    }
    out
}

fn walk_for_playtimes(value: &keyvalues_parser::Value, into: &mut HashMap<u64, u64>) {
    use keyvalues_parser::Value;
    let Value::Obj(obj) = value else {
        return;
    };
    for (k, values) in obj.iter() {
        if k.eq_ignore_ascii_case("apps") {
            for v in values {
                let Value::Obj(apps) = v else {
                    continue;
                };
                for (appid_s, appid_values) in apps.iter() {
                    let Ok(appid) = appid_s.parse::<u64>() else {
                        continue;
                    };
                    for av in appid_values {
                        let Value::Obj(fields) = av else {
                            continue;
                        };
                        for (fk, fvs) in fields.iter() {
                            if fk.eq_ignore_ascii_case("Playtime") {
                                for fv in fvs {
                                    if let Value::Str(s) = fv {
                                        if let Ok(m) = s.parse::<u64>() {
                                            into.entry(appid)
                                                .and_modify(|e| *e = (*e).max(m))
                                                .or_insert(m);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else {
            for v in values {
                walk_for_playtimes(v, into);
            }
        }
    }
}

// ============================================================================
// Database
// ============================================================================

fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS games (
            id           TEXT PRIMARY KEY,
            source       TEXT NOT NULL,
            app_id       TEXT NOT NULL,
            name         TEXT NOT NULL,
            install_path TEXT NOT NULL,
            launch_url   TEXT NOT NULL,
            size_bytes   INTEGER NOT NULL DEFAULT 0,
            cover_url    TEXT,
            hero_url     TEXT,
            logo_url     TEXT,
            cover_local  TEXT,
            hero_local   TEXT,
            logo_local   TEXT,
            first_seen   INTEGER NOT NULL,
            last_seen    INTEGER NOT NULL,
            removed      INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS play_sessions (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            game_id         TEXT NOT NULL,
            started_at      INTEGER NOT NULL,
            ended_at        INTEGER,
            duration_seconds INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_sessions_game ON play_sessions(game_id);

        CREATE TABLE IF NOT EXISTS meta (
            key   TEXT PRIMARY KEY,
            value TEXT
        );
        "#,
    )?;
    // Idempotent migrations. ALTER TABLE ADD COLUMN fails after the first
    // run — we swallow the "duplicate column" error.
    let _ = conn.execute(
        "ALTER TABLE games ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0",
        [],
    );
    Ok(())
}

/// Any session that was left open (e.g., launcher was killed) — close it
/// with ended_at = started_at so it contributes zero time (safer than guessing).
fn close_orphan_sessions(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE play_sessions
            SET ended_at = started_at, duration_seconds = 0
          WHERE ended_at IS NULL",
        [],
    )?;
    Ok(())
}

fn db_get_meta(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM meta WHERE key = ?1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .optional()
}

fn db_set_meta(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO meta(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[derive(Clone, Debug)]
struct DetectedGame {
    id: String,
    source: String,
    app_id: String,
    name: String,
    install_path: String,
    launch_url: String,
    size_bytes: u64,
    cover_url: Option<String>,
    hero_url: Option<String>,
    logo_url: Option<String>,
}

fn db_upsert_games(conn: &mut Connection, games: &[DetectedGame]) -> rusqlite::Result<()> {
    let now = now_unix();
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO games (id, source, app_id, name, install_path, launch_url,
                                size_bytes, cover_url, hero_url, logo_url,
                                first_seen, last_seen, removed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, 0)
             ON CONFLICT(id) DO UPDATE SET
                 source       = excluded.source,
                 app_id       = excluded.app_id,
                 name         = excluded.name,
                 install_path = excluded.install_path,
                 launch_url   = excluded.launch_url,
                 size_bytes   = excluded.size_bytes,
                 cover_url    = COALESCE(excluded.cover_url, games.cover_url),
                 hero_url     = COALESCE(excluded.hero_url,  games.hero_url),
                 logo_url     = COALESCE(excluded.logo_url,  games.logo_url),
                 last_seen    = excluded.last_seen,
                 removed      = 0",
        )?;
        for g in games {
            stmt.execute(params![
                g.id,
                g.source,
                g.app_id,
                g.name,
                g.install_path,
                g.launch_url,
                g.size_bytes as i64,
                g.cover_url,
                g.hero_url,
                g.logo_url,
                now,
            ])?;
        }
    }
    tx.commit()
}

fn db_mark_missing_removed(conn: &Connection, present_ids: &[String]) -> rusqlite::Result<()> {
    if present_ids.is_empty() {
        conn.execute("UPDATE games SET removed = 1", [])?;
        return Ok(());
    }
    let placeholders = vec!["?"; present_ids.len()].join(",");
    let sql = format!(
        "UPDATE games SET removed = 1 WHERE id NOT IN ({placeholders})",
        placeholders = placeholders
    );
    let params: Vec<&dyn rusqlite::ToSql> =
        present_ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    conn.execute(&sql, &params[..])?;
    Ok(())
}

fn db_update_local_asset(
    conn: &Connection,
    game_id: &str,
    kind: &str,
    local_path: &str,
) -> rusqlite::Result<()> {
    let col = match kind {
        "cover" => "cover_local",
        "hero" => "hero_local",
        "logo" => "logo_local",
        _ => return Ok(()),
    };
    let sql = format!("UPDATE games SET {} = ?1 WHERE id = ?2", col);
    conn.execute(&sql, params![local_path, game_id])?;
    Ok(())
}

fn db_list_games(conn: &Connection) -> rusqlite::Result<Vec<Game>> {
    let mut stmt = conn.prepare(
        "SELECT g.id, g.source, g.app_id, g.name, g.install_path, g.launch_url,
                g.size_bytes,
                g.cover_url, g.hero_url, g.logo_url,
                g.cover_local, g.hero_local, g.logo_local,
                COALESCE((SELECT SUM(duration_seconds) FROM play_sessions
                           WHERE game_id = g.id AND duration_seconds IS NOT NULL), 0)
                  AS total_seconds
           FROM games g
          WHERE g.removed = 0 AND g.hidden = 0
          ORDER BY LOWER(g.name)",
    )?;
    let rows = stmt.query_map([], |r| {
        let total_seconds: i64 = r.get(13)?;
        Ok(Game {
            id: r.get(0)?,
            source: r.get(1)?,
            app_id: r.get(2)?,
            name: r.get(3)?,
            install_path: r.get(4)?,
            launch_url: r.get(5)?,
            size_bytes: r.get::<_, i64>(6)? as u64,
            cover_url: r.get(7)?,
            hero_url: r.get(8)?,
            logo_url: r.get(9)?,
            cover_local: r.get(10)?,
            hero_local: r.get(11)?,
            logo_local: r.get(12)?,
            playtime_minutes: (total_seconds.max(0) as u64) / 60,
        })
    })?;
    let mut out = Vec::new();
    for g in rows {
        out.push(g?);
    }
    Ok(out)
}

fn db_get_game(conn: &Connection, id: &str) -> rusqlite::Result<Option<Game>> {
    let mut stmt = conn.prepare(
        "SELECT g.id, g.source, g.app_id, g.name, g.install_path, g.launch_url,
                g.size_bytes,
                g.cover_url, g.hero_url, g.logo_url,
                g.cover_local, g.hero_local, g.logo_local,
                COALESCE((SELECT SUM(duration_seconds) FROM play_sessions
                           WHERE game_id = g.id AND duration_seconds IS NOT NULL), 0)
           FROM games g
          WHERE g.id = ?1",
    )?;
    stmt.query_row(params![id], |r| {
        let total_seconds: i64 = r.get(13)?;
        Ok(Game {
            id: r.get(0)?,
            source: r.get(1)?,
            app_id: r.get(2)?,
            name: r.get(3)?,
            install_path: r.get(4)?,
            launch_url: r.get(5)?,
            size_bytes: r.get::<_, i64>(6)? as u64,
            cover_url: r.get(7)?,
            hero_url: r.get(8)?,
            logo_url: r.get(9)?,
            cover_local: r.get(10)?,
            hero_local: r.get(11)?,
            logo_local: r.get(12)?,
            playtime_minutes: (total_seconds.max(0) as u64) / 60,
        })
    })
    .optional()
}

fn db_open_session(conn: &Connection, game_id: &str, started_at: i64) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO play_sessions (game_id, started_at) VALUES (?1, ?2)",
        params![game_id, started_at],
    )?;
    Ok(conn.last_insert_rowid())
}

fn db_close_session(
    conn: &Connection,
    session_id: i64,
    ended_at: i64,
    duration_seconds: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE play_sessions
            SET ended_at = ?1, duration_seconds = ?2
          WHERE id = ?3",
        params![ended_at, duration_seconds, session_id],
    )?;
    Ok(())
}

fn db_total_minutes(conn: &Connection, game_id: &str) -> rusqlite::Result<u64> {
    let total: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(duration_seconds), 0) FROM play_sessions
              WHERE game_id = ?1 AND duration_seconds IS NOT NULL",
            params![game_id],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0);
    Ok((total.max(0) as u64) / 60)
}

/// Insert a single synthetic session representing Steam's existing lifetime
/// playtime for a game. Run once on first scan so existing hours are preserved.
fn db_seed_steam_playtime(
    conn: &Connection,
    steam_games: &[(String, u64)], // (game_id, steam_appid)
    playtimes: &HashMap<u64, u64>,
) -> rusqlite::Result<()> {
    let now = now_unix();
    for (game_id, appid) in steam_games {
        let Some(mins) = playtimes.get(appid).copied() else {
            continue;
        };
        if mins == 0 {
            continue;
        }
        let duration_s = (mins as i64) * 60;
        conn.execute(
            "INSERT INTO play_sessions (game_id, started_at, ended_at, duration_seconds)
             VALUES (?1, ?2, ?2, ?3)",
            params![game_id, now, duration_s],
        )?;
    }
    Ok(())
}

// ============================================================================
// Epic art lookup (Steam Community search — Epic's own APIs are CF-blocked)
// ============================================================================

#[derive(Clone, Debug, Default)]
struct EpicArt {
    cover: Option<String>,
    hero: Option<String>,
    logo: Option<String>,
}

static EPIC_CACHE: OnceLock<TokioMutex<HashMap<String, EpicArt>>> = OnceLock::new();

fn epic_cache() -> &'static TokioMutex<HashMap<String, EpicArt>> {
    EPIC_CACHE.get_or_init(|| TokioMutex::new(HashMap::new()))
}

async fn fetch_epic_art(client: &reqwest::Client, name: &str) -> EpicArt {
    {
        let cache = epic_cache().lock().await;
        if let Some(cached) = cache.get(name) {
            return cached.clone();
        }
    }

    let url = format!(
        "https://steamcommunity.com/actions/SearchApps/{}",
        urlencoding::encode(name)
    );
    let art = match client.get(&url).send().await {
        Ok(resp) => match resp.json::<Vec<serde_json::Value>>().await {
            Ok(results) => {
                let target = normalize_title(name);
                let matched = results.iter().find(|r| {
                    r.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| normalize_title(s) == target)
                        .unwrap_or(false)
                });
                match matched.and_then(|m| m.get("appid")).and_then(|a| a.as_str()) {
                    Some(appid) if !appid.is_empty() => EpicArt {
                        cover: Some(steam_cdn(appid, "library_600x900.jpg")),
                        hero: Some(steam_cdn(appid, "library_hero.jpg")),
                        logo: Some(steam_cdn(appid, "logo.png")),
                    },
                    _ => EpicArt::default(),
                }
            }
            Err(_) => EpicArt::default(),
        },
        Err(_) => EpicArt::default(),
    };

    let mut cache = epic_cache().lock().await;
    cache.insert(name.to_string(), art.clone());
    art
}

// ============================================================================
// Scan pipeline
// ============================================================================

async fn run_scan(
    app: AppHandle,
    db: Arc<Mutex<Connection>>,
    assets_dir: PathBuf,
    http: reqwest::Client,
) -> Result<(), String> {
    let emit = |stage: &str, done: u32, total: u32| {
        let _ = app.emit(
            "scan:progress",
            ScanProgress {
                stage: stage.to_string(),
                done,
                total,
            },
        );
    };

    // --- Stage 1: detect games ---
    emit("Detecting games", 0, 0);
    let (detected, steam_playtimes) = tauri::async_runtime::spawn_blocking(|| {
        let detected = game_detector::find_all_games();
        let pt = load_steam_playtimes();
        (detected, pt)
    })
    .await
    .map_err(|e| format!("detection panicked: {e}"))?;

    // Build DetectedGame list + collect Epic names needing art lookup
    let mut games: Vec<DetectedGame> = Vec::new();
    let mut epic_indices: Vec<(usize, String)> = Vec::new();
    let mut steam_mapping: Vec<(String, u64)> = Vec::new(); // (game_id, steam appid)

    for entry in detected {
        match entry {
            game_detector::InstalledGame::Steam(app_st) => {
                if is_excluded_steam_entry(app_st.appid, &app_st.name) {
                    continue;
                }
                let appid_s = app_st.appid.to_string();
                let id = format!("steam:{}", app_st.appid);
                steam_mapping.push((id.clone(), app_st.appid));
                games.push(DetectedGame {
                    id,
                    source: "Steam".into(),
                    app_id: appid_s.clone(),
                    name: app_st.name.clone(),
                    install_path: app_st.game_path.clone(),
                    launch_url: format!("steam://rungameid/{}", app_st.appid),
                    size_bytes: app_st.SizeOnDisk as u64,
                    cover_url: Some(steam_cdn(&appid_s, "library_600x900.jpg")),
                    hero_url: Some(steam_cdn(&appid_s, "library_hero.jpg")),
                    logo_url: Some(steam_cdn(&appid_s, "logo.png")),
                });
            }
            game_detector::InstalledGame::EpicGames(m) => {
                let display = if m.display_name.is_empty() {
                    m.app_name.clone()
                } else {
                    m.display_name.clone()
                };
                let idx = games.len();
                epic_indices.push((idx, display.clone()));
                let initial_cover = if m.vault_thumbnail_url.is_empty() {
                    None
                } else {
                    Some(m.vault_thumbnail_url.clone())
                };
                games.push(DetectedGame {
                    id: format!("epic:{}", m.app_name),
                    source: "Epic".into(),
                    app_id: m.app_name.clone(),
                    name: display,
                    install_path: m.install_location.clone(),
                    launch_url: format!(
                        "com.epicgames.launcher://apps/{}?action=launch&silent=true",
                        m.app_name
                    ),
                    size_bytes: m.install_size.max(0) as u64,
                    cover_url: initial_cover,
                    hero_url: None,
                    logo_url: None,
                });
            }
            _ => {}
        }
    }

    // --- Stage 2: resolve Epic art ---
    if !epic_indices.is_empty() {
        let total = epic_indices.len() as u32;
        emit("Resolving artwork", 0, total);

        let mut set = tokio::task::JoinSet::new();
        for (idx, name) in epic_indices.into_iter() {
            let client = http.clone();
            set.spawn(async move {
                let art = fetch_epic_art(&client, &name).await;
                (idx, art)
            });
        }
        let mut done = 0u32;
        while let Some(res) = set.join_next().await {
            if let Ok((idx, art)) = res {
                if games[idx].cover_url.is_none() {
                    games[idx].cover_url = art.cover;
                }
                if games[idx].hero_url.is_none() {
                    games[idx].hero_url = art.hero;
                }
                if games[idx].logo_url.is_none() {
                    games[idx].logo_url = art.logo;
                }
            }
            done += 1;
            emit("Resolving artwork", done, total);
        }
    }

    // --- Stage 3: upsert DB + backfill Steam playtime on first run ---
    emit("Saving library", 0, 0);
    let games_for_db = games.clone();
    let ids_present: Vec<String> = games.iter().map(|g| g.id.clone()).collect();
    let steam_mapping_for_db = steam_mapping.clone();
    let db_for_save = db.clone();
    tauri::async_runtime::spawn_blocking(move || -> rusqlite::Result<()> {
        let mut conn = db_for_save.lock().unwrap();
        db_upsert_games(&mut conn, &games_for_db)?;
        db_mark_missing_removed(&conn, &ids_present)?;

        // First-run backfill: only if we haven't done it before.
        if db_get_meta(&conn, "steam_playtime_imported")?.is_none() {
            db_seed_steam_playtime(&conn, &steam_mapping_for_db, &steam_playtimes)?;
            db_set_meta(&conn, "steam_playtime_imported", "1")?;
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("db save task panicked: {e}"))?
    .map_err(|e| format!("db save failed: {e}"))?;

    // --- Stage 4: prefetch assets to local disk ---
    let mut todo: Vec<(String, &'static str, String)> = Vec::new();
    for g in &games {
        for (kind, url_opt) in [
            ("cover", &g.cover_url),
            ("hero", &g.hero_url),
            ("logo", &g.logo_url),
        ] {
            let Some(url) = url_opt else { continue };
            let filename = format!("{}_{}.{}", safe_id(&g.id), kind, guess_ext(url));
            let local_path = assets_dir.join(&filename);
            if local_path.exists() && fs::metadata(&local_path).map(|m| m.len()).unwrap_or(0) > 0 {
                // Already cached; make sure DB knows.
                let db_copy = db.clone();
                let id_c = g.id.clone();
                let kind_c = kind;
                let local_str = local_path.to_string_lossy().to_string();
                tauri::async_runtime::spawn_blocking(move || {
                    let conn = db_copy.lock().unwrap();
                    let _ = db_update_local_asset(&conn, &id_c, kind_c, &local_str);
                })
                .await
                .ok();
                continue;
            }
            todo.push((g.id.clone(), kind, url.clone()));
        }
    }

    let total = todo.len() as u32;
    if total > 0 {
        emit("Downloading art", 0, total);
        let semaphore = Arc::new(Semaphore::new(6));
        let mut set = tokio::task::JoinSet::new();
        for (game_id, kind, url) in todo {
            let semaphore = semaphore.clone();
            let http = http.clone();
            let assets_dir = assets_dir.clone();
            let db = db.clone();
            set.spawn(async move {
                let _permit = semaphore.acquire_owned().await.ok();
                let filename = format!("{}_{}.{}", safe_id(&game_id), kind, guess_ext(&url));
                let local_path = assets_dir.join(&filename);
                match download_to(&http, &url, &local_path).await {
                    Ok(()) => {
                        let local_str = local_path.to_string_lossy().to_string();
                        let db_copy = db.clone();
                        let id_c = game_id.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            let conn = db_copy.lock().unwrap();
                            let _ = db_update_local_asset(&conn, &id_c, kind, &local_str);
                        })
                        .await
                        .ok();
                    }
                    Err(_) => { /* skip silently — fallback to remote URL in UI */ }
                }
            });
        }
        let mut done = 0u32;
        while let Some(_) = set.join_next().await {
            done += 1;
            emit("Downloading art", done, total);
        }
    }

    let _ = app.emit("scan:done", ());
    Ok(())
}

async fn download_to(
    client: &reqwest::Client,
    url: &str,
    out: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()).into());
    }
    let bytes = resp.bytes().await?;
    if bytes.is_empty() {
        return Err("empty body".into());
    }
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(out, &bytes)?;
    Ok(())
}

// ============================================================================
// Playtime watcher (process-based)
// ============================================================================

fn start_playtime_watcher(
    app: AppHandle,
    db: Arc<Mutex<Connection>>,
    watchers: Arc<Mutex<HashSet<String>>>,
    game: Game,
) {
    {
        let mut w = watchers.lock().unwrap();
        if w.contains(&game.id) {
            return;
        }
        w.insert(game.id.clone());
    }

    tauri::async_runtime::spawn(async move {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, UpdateKind};

        let install_path_lc = game.install_path.to_lowercase();
        let install_path_lc_norm = install_path_lc.replace('\\', "/");

        let mut sys = sysinfo::System::new();
        let refresh_kind =
            ProcessRefreshKind::new().with_exe(UpdateKind::Always);

        let mut session_id: Option<i64> = None;
        let mut session_started: Option<i64> = None;

        // Give the game up to 90s to spawn its first process.
        let start_deadline = std::time::Instant::now() + Duration::from_secs(90);
        // Once running, give it 20s of "no process seen" grace before closing.
        let mut last_seen_running: Option<std::time::Instant> = None;
        let grace = Duration::from_secs(20);

        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            sys.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind);

            let running = sys.processes().values().any(|p| {
                let Some(exe) = p.exe() else {
                    return false;
                };
                let Some(s) = exe.to_str() else {
                    return false;
                };
                let s = s.to_lowercase().replace('\\', "/");
                s.starts_with(&install_path_lc_norm)
            });

            if running {
                last_seen_running = Some(std::time::Instant::now());
                if session_id.is_none() {
                    let now = now_unix();
                    let db_c = db.clone();
                    let id = game.id.clone();
                    let opened = tauri::async_runtime::spawn_blocking(move || {
                        let conn = db_c.lock().unwrap();
                        db_open_session(&conn, &id, now).ok()
                    })
                    .await
                    .ok()
                    .flatten();
                    session_id = opened;
                    session_started = Some(now);
                }
            } else {
                if session_id.is_some() {
                    // Wait for grace before considering the session closed.
                    if let Some(t) = last_seen_running {
                        if t.elapsed() < grace {
                            continue;
                        }
                    }
                    let now = now_unix();
                    let duration = now - session_started.unwrap_or(now);
                    if let Some(sid) = session_id.take() {
                        let db_c = db.clone();
                        let _ = tauri::async_runtime::spawn_blocking(move || {
                            let conn = db_c.lock().unwrap();
                            db_close_session(&conn, sid, now, duration.max(0))
                        })
                        .await;
                    }
                    // Emit updated total.
                    let db_c = db.clone();
                    let id_c = game.id.clone();
                    let total = tauri::async_runtime::spawn_blocking(move || {
                        let conn = db_c.lock().unwrap();
                        db_total_minutes(&conn, &id_c).unwrap_or(0)
                    })
                    .await
                    .unwrap_or(0);
                    let _ = app.emit(
                        "playtime:updated",
                        PlaytimeUpdate {
                            game_id: game.id.clone(),
                            playtime_minutes: total,
                        },
                    );
                    break;
                }
                // No session started yet; if we've waited long enough, give up.
                if std::time::Instant::now() > start_deadline {
                    break;
                }
            }
        }

        // Cleanup watcher entry
        let mut w = watchers.lock().unwrap();
        w.remove(&game.id);
    });
}

// ============================================================================
// Tauri commands
// ============================================================================

#[tauri::command]
async fn init_scan(app: AppHandle, state: State<'_, AppStateInner>) -> Result<(), String> {
    let lock = state.scan_lock.clone();
    let _guard = lock.try_lock().map_err(|_| "scan already running".to_string())?;
    let db = state.db.clone();
    let assets_dir = state.assets_dir.clone();
    let http = state.http.clone();
    run_scan(app, db, assets_dir, http).await
}

#[tauri::command]
async fn list_games(state: State<'_, AppStateInner>) -> Result<Vec<Game>, String> {
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db.lock().unwrap();
        db_list_games(&conn).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("db task panicked: {e}"))?
}

#[tauri::command]
async fn launch_game(
    id: String,
    app: AppHandle,
    state: State<'_, AppStateInner>,
) -> Result<(), String> {
    let game = {
        let db = state.db.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let conn = db.lock().unwrap();
            db_get_game(&conn, &id).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("db task panicked: {e}"))??
        .ok_or_else(|| "game not found".to_string())?
    };

    app.opener()
        .open_url(&game.launch_url, None::<&str>)
        .map_err(|e| format!("launch failed: {e}"))?;

    start_playtime_watcher(
        app.clone(),
        state.db.clone(),
        state.watchers.clone(),
        game,
    );
    Ok(())
}

#[tauri::command]
async fn hide_game(id: String, state: State<'_, AppStateInner>) -> Result<(), String> {
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let conn = db.lock().unwrap();
        conn.execute("UPDATE games SET hidden = 1 WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| format!("db task panicked: {e}"))?
}

#[tauri::command]
async fn reset_playtime(
    id: String,
    app: AppHandle,
    state: State<'_, AppStateInner>,
) -> Result<(), String> {
    let id_for_db = id.clone();
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let conn = db.lock().unwrap();
        conn.execute(
            "DELETE FROM play_sessions WHERE game_id = ?1",
            params![id_for_db],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| format!("db task panicked: {e}"))??;

    let _ = app.emit(
        "playtime:updated",
        PlaytimeUpdate {
            game_id: id,
            playtime_minutes: 0,
        },
    );
    Ok(())
}

#[tauri::command]
async fn uninstall_game(
    id: String,
    app: AppHandle,
    state: State<'_, AppStateInner>,
) -> Result<(), String> {
    let game = {
        let db = state.db.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let conn = db.lock().unwrap();
            db_get_game(&conn, &id).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("db task panicked: {e}"))??
        .ok_or_else(|| "game not found".to_string())?
    };
    let url = match game.source.as_str() {
        "Steam" => format!("steam://uninstall/{}", game.app_id),
        "Epic" => format!(
            "com.epicgames.launcher://apps/{}?action=uninstall",
            game.app_id
        ),
        other => return Err(format!("unknown source: {}", other)),
    };
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| format!("uninstall failed: {e}"))?;
    Ok(())
}

#[tauri::command]
async fn open_data_folder(
    app: AppHandle,
    state: State<'_, AppStateInner>,
) -> Result<(), String> {
    let path = state
        .assets_dir
        .parent()
        .ok_or_else(|| "no app data parent".to_string())?
        .to_string_lossy()
        .to_string();
    app.opener()
        .open_path(&path, None::<&str>)
        .map_err(|e| format!("open path failed: {e}"))?;
    Ok(())
}

#[tauri::command]
async fn get_prefs(state: State<'_, AppStateInner>) -> Result<Prefs, String> {
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<Prefs, String> {
        let conn = db.lock().unwrap();
        let show_steam = db_get_meta(&conn, "show_steam")
            .map_err(|e| e.to_string())?
            .map(|v| v != "0")
            .unwrap_or(true);
        let show_epic = db_get_meta(&conn, "show_epic")
            .map_err(|e| e.to_string())?
            .map(|v| v != "0")
            .unwrap_or(true);
        let fullscreen = db_get_meta(&conn, "fullscreen")
            .map_err(|e| e.to_string())?
            .map(|v| v != "0")
            .unwrap_or(true);
        Ok(Prefs {
            show_steam,
            show_epic,
            fullscreen,
        })
    })
    .await
    .map_err(|e| format!("db task panicked: {e}"))?
}

#[tauri::command]
async fn set_pref(
    key: String,
    value: String,
    state: State<'_, AppStateInner>,
) -> Result<(), String> {
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let conn = db.lock().unwrap();
        db_set_meta(&conn, &key, &value).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("db task panicked: {e}"))?
}

#[tauri::command]
async fn set_fullscreen(
    enabled: bool,
    app: AppHandle,
    state: State<'_, AppStateInner>,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_fullscreen(enabled)
            .map_err(|e| format!("set_fullscreen failed: {e}"))?;
    }
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let conn = db.lock().unwrap();
        db_set_meta(&conn, "fullscreen", if enabled { "1" } else { "0" })
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("db task panicked: {e}"))?
}

// ============================================================================
// Entry
// ============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data)?;
            let assets_dir = app_data.join("assets");
            fs::create_dir_all(&assets_dir)?;
            let db_path = app_data.join("library.db");
            let conn = Connection::open(&db_path)?;
            init_schema(&conn)?;
            close_orphan_sessions(&conn)?;

            // Apply window state from stored prefs (defaults to fullscreen
            // if no pref set yet). Config has `visible: false` so we can
            // set fullscreen before the window ever paints, avoiding a
            // windowed flash.
            let fullscreen = db_get_meta(&conn, "fullscreen")
                .ok()
                .flatten()
                .map(|v| v != "0")
                .unwrap_or(true);
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_fullscreen(fullscreen);
                let _ = window.show();
            }

            let http = reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .user_agent(
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) games-launcher/0.1",
                )
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());

            app.manage(AppStateInner {
                db: Arc::new(Mutex::new(conn)),
                assets_dir,
                watchers: Arc::new(Mutex::new(HashSet::new())),
                http,
                scan_lock: Arc::new(TokioMutex::new(())),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            init_scan,
            list_games,
            launch_game,
            hide_game,
            reset_playtime,
            uninstall_game,
            open_data_folder,
            get_prefs,
            set_pref,
            set_fullscreen
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
