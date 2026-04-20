use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::Mutex;
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub name: String,
    pub source: &'static str,
    pub app_id: String,
    pub install_path: String,
    pub launch_url: String,
    pub cover_url: Option<String>,
    pub hero_url: Option<String>,
    pub logo_url: Option<String>,
    pub size_bytes: u64,
    pub playtime_minutes: Option<u64>,
}

fn steam_cdn(appid: &str, asset: &str) -> String {
    format!(
        "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/{}",
        appid, asset
    )
}

// AppIDs for Steam runtime / redistributable / compatibility tools that aren't
// real games. We also filter by name below to catch ones we don't list here.
const EXCLUDED_STEAM_APPIDS: &[u64] = &[
    228980,  // Steamworks Common Redistributables
    250820,  // SteamVR
    323910,  // SteamVR Performance Test
    1070560, // Steam Linux Runtime
    1391110, // Steam Linux Runtime - Soldier
    1628350, // Steam Linux Runtime - Sniper
    1493710, // Proton Experimental
    1887720, // Proton 7.0
    1420170, // Proton Hotfix
    2348590, // Proton 8.0
    2180100, // Proton Next
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

// ========== Steam local playtime ==========

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

/// Read all per-user `localconfig.vdf` files and return a map of
/// Steam appid -> lifetime playtime in minutes. Takes the max across users.
fn load_steam_playtimes() -> HashMap<u64, u64> {
    let mut out: HashMap<u64, u64> = HashMap::new();
    let Some(steam) = steam_install_path() else {
        return out;
    };
    let userdata = steam.join("userdata");
    let Ok(entries) = std::fs::read_dir(&userdata) else {
        return out;
    };

    for entry in entries.flatten() {
        let config_path = entry.path().join("config").join("localconfig.vdf");
        if !config_path.is_file() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&config_path) else {
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

// ========== Epic art lookup via Steam Community search ==========

#[derive(Clone, Debug, Default)]
struct EpicArt {
    cover: Option<String>,
    hero: Option<String>,
    logo: Option<String>,
}

static EPIC_CACHE: OnceLock<Mutex<HashMap<String, EpicArt>>> = OnceLock::new();

fn epic_cache() -> &'static Mutex<HashMap<String, EpicArt>> {
    EPIC_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
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

// ========== Tauri commands ==========

#[tauri::command]
async fn list_games() -> Result<Vec<Game>, String> {
    let (detected, playtimes) = tauri::async_runtime::spawn_blocking(|| {
        let detected = game_detector::find_all_games();
        let playtimes = load_steam_playtimes();
        (detected, playtimes)
    })
    .await
    .map_err(|e| format!("detection task panicked: {e}"))?;

    let mut games: Vec<Game> = Vec::new();
    let mut epic_lookups: Vec<(usize, String)> = Vec::new();

    for entry in detected {
        match entry {
            game_detector::InstalledGame::Steam(app) => {
                if is_excluded_steam_entry(app.appid, &app.name) {
                    continue;
                }
                let appid = app.appid.to_string();
                games.push(Game {
                    id: format!("steam:{}", app.appid),
                    name: app.name.clone(),
                    source: "Steam",
                    app_id: appid.clone(),
                    install_path: app.game_path.clone(),
                    launch_url: format!("steam://rungameid/{}", app.appid),
                    cover_url: Some(steam_cdn(&appid, "library_600x900.jpg")),
                    hero_url: Some(steam_cdn(&appid, "library_hero.jpg")),
                    logo_url: Some(steam_cdn(&appid, "logo.png")),
                    size_bytes: app.SizeOnDisk as u64,
                    playtime_minutes: playtimes.get(&app.appid).copied(),
                });
            }
            game_detector::InstalledGame::EpicGames(m) => {
                let display = if m.display_name.is_empty() {
                    m.app_name.clone()
                } else {
                    m.display_name.clone()
                };
                let initial_cover = if m.vault_thumbnail_url.is_empty() {
                    None
                } else {
                    Some(m.vault_thumbnail_url.clone())
                };
                let idx = games.len();
                epic_lookups.push((idx, display.clone()));
                games.push(Game {
                    id: format!("epic:{}", m.app_name),
                    name: display,
                    source: "Epic",
                    app_id: m.app_name.clone(),
                    install_path: m.install_location.clone(),
                    launch_url: format!(
                        "com.epicgames.launcher://apps/{}?action=launch&silent=true",
                        m.app_name
                    ),
                    cover_url: initial_cover,
                    hero_url: None,
                    logo_url: None,
                    size_bytes: m.install_size.max(0) as u64,
                    playtime_minutes: None,
                });
            }
            _ => {}
        }
    }

    if !epic_lookups.is_empty() {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(6))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) games-launcher/0.1")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let mut set = tokio::task::JoinSet::new();
        for (idx, name) in epic_lookups.into_iter() {
            let client = client.clone();
            set.spawn(async move {
                let art = fetch_epic_art(&client, &name).await;
                (idx, art)
            });
        }
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
        }
    }

    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(games)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![list_games])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
