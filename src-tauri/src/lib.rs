use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::Mutex;

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
}

fn steam_cdn(appid: u64, asset: &str) -> String {
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

// ========== Epic art lookup ==========

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

// Extract first image URL matching any of the given Epic keyImage types.
fn pick_key_image(images: Option<&serde_json::Value>, types: &[&str]) -> Option<String> {
    let arr = images?.as_array()?;
    for t in types {
        for img in arr {
            if img.get("type").and_then(|v| v.as_str()) == Some(*t) {
                if let Some(url) = img.get("url").and_then(|v| v.as_str()) {
                    return Some(url.to_string());
                }
            }
        }
    }
    None
}

async fn fetch_epic_art(client: &reqwest::Client, name: &str) -> EpicArt {
    {
        let cache = epic_cache().lock().await;
        if let Some(cached) = cache.get(name) {
            return cached.clone();
        }
    }

    let body = serde_json::json!({
        "query": "query q($k: String!) { Catalog { searchStore(keywords: $k, country: \"US\", locale: \"en-US\", count: 5) { elements { title id namespace keyImages { type url } } } } }",
        "variables": { "k": name }
    });

    let art = match client
        .post("https://graphql.epicgames.com/graphql")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(json) => {
                let elements = json.pointer("/data/Catalog/searchStore/elements");
                // Prefer an element whose title case-insensitively matches the
                // search name; otherwise fall back to the first element.
                let picked = elements.and_then(|v| v.as_array()).and_then(|arr| {
                    let lname = name.to_lowercase();
                    arr.iter()
                        .find(|el| {
                            el.get("title")
                                .and_then(|t| t.as_str())
                                .map(|s| s.to_lowercase() == lname)
                                .unwrap_or(false)
                        })
                        .or_else(|| arr.first())
                });
                let images = picked.and_then(|e| e.get("keyImages"));
                EpicArt {
                    cover: pick_key_image(
                        images,
                        &["OfferImageTall", "DieselStoreFrontTall", "Thumbnail", "VaultClosed"],
                    ),
                    hero: pick_key_image(
                        images,
                        &[
                            "OfferImageWide",
                            "DieselStoreFrontWide",
                            "DieselGameBoxWide",
                            "Featured",
                        ],
                    ),
                    logo: pick_key_image(images, &["ProductLogo", "AndroidIcon"]),
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
    let detected = tauri::async_runtime::spawn_blocking(game_detector::find_all_games)
        .await
        .map_err(|e| format!("detection task panicked: {e}"))?;

    let mut games: Vec<Game> = Vec::new();
    // (index in `games`, search name) for each Epic entry that needs art lookup.
    let mut epic_lookups: Vec<(usize, String)> = Vec::new();

    for entry in detected {
        match entry {
            game_detector::InstalledGame::Steam(app) => {
                if is_excluded_steam_entry(app.appid, &app.name) {
                    continue;
                }
                games.push(Game {
                    id: format!("steam:{}", app.appid),
                    name: app.name.clone(),
                    source: "Steam",
                    app_id: app.appid.to_string(),
                    install_path: app.game_path.clone(),
                    launch_url: format!("steam://rungameid/{}", app.appid),
                    cover_url: Some(steam_cdn(app.appid, "library_600x900.jpg")),
                    hero_url: Some(steam_cdn(app.appid, "library_hero.jpg")),
                    logo_url: Some(steam_cdn(app.appid, "logo.png")),
                    size_bytes: app.SizeOnDisk as u64,
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
                });
            }
            _ => {}
        }
    }

    // Concurrently fetch artwork for Epic games from the public store GraphQL.
    if !epic_lookups.is_empty() {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(6))
            .user_agent("games-launcher/0.1 (+https://github.com/)")
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
