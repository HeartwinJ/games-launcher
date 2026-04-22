# Games Launcher

A minimal, controller-friendly Windows game launcher that aggregates your installed Steam and Epic Games library into a single, fullscreen-capable grid. Built with Tauri 2, SvelteKit, and SQLite.

The goal is a "big picture"-style experience on top of the libraries you already have — no account sign-in, no store front, no clutter. Just your installed games, beautiful artwork, and a short path from launch to play.

## Features

### Library
- **Unified detection** — scans Steam (via local `.acf` manifests) and Epic (via the Epic Games Launcher manifests) through the `game-detector` crate.
- **Artwork for everything** — Steam games get cover, landscape hero, and transparent wordmark straight from Steam's CDN. Epic games are matched against Steam Community's public search by normalized title, so multiplatform titles (GTA V, Cyberpunk, The Finals, etc.) get the same rich art treatment.
- **Utility filtering** — Steamworks Redistributables, Proton, Steam Linux Runtime, and SteamVR are excluded automatically so the grid stays clean.
- **Local SQLite library** — installed games, remote and cached asset paths, and first/last-seen timestamps live in `library.db` under the app's data directory. Games that disappear between scans are soft-deleted, not dropped, so their history is preserved if they come back.
- **Asset prefetch** — on scan, every cover/hero/logo is downloaded into `assets/` with a concurrency cap of 6, then served locally via Tauri's asset protocol. Offline launches are instant.

### Playtime tracking
- Native process-based session tracking: after you click Play, a `sysinfo` watcher polls every 5 seconds for any process whose executable path is inside the game's install directory. A session opens when one appears and closes 20 seconds after the last one exits (grace window absorbs anti-cheat restarts).
- Sessions are stored in the `play_sessions` table; the displayed playtime is a live sum from that table.
- **First-run backfill** — on the initial scan, the launcher imports existing Steam playtime from `localconfig.vdf` as a single historical session per game, so you don't lose your existing hours. Every session after that is recorded natively.
- Orphaned sessions (left open because the launcher was killed mid-session) are closed with zero duration at the next startup.

### Interface
- **Splash + scan pipeline** — glass splash panel with per-stage progress ("Detecting games", "Resolving artwork", "Downloading art N/M") runs while the backend scans and prefetches.
- **Hero + carousel layout** — full-width hero image for the focused game fills the top half; a horizontal carousel of portrait cards sits at the bottom.
- **Ambient backdrop** — the hero doubles as a soft-blurred viewport backdrop that cross-fades on selection change.
- **Glassmorphic theme** — frosted-glass Play button, settings button, and a single compact stats capsule showing `[store icon] | [size] | [playtime]` with the real Steam and Epic Games logos as store marks.
- **Keyboard + Xbox controller navigation** — arrows/left-stick/D-pad to move, `Enter`/A to launch, filter with `1/2/3`, `Home`/`End` to jump.

### Launching
- Steam games launch via `steam://rungameid/<appid>`.
- Epic games launch via `com.epicgames.launcher://apps/<app_name>?action=launch&silent=true`.
- Both launches happen inside the Rust backend so the playtime watcher starts automatically.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                         SvelteKit UI                         │
│   Splash · Hero · Carousel · Keyboard/Gamepad · Events       │
└──────────────────────────┬───────────────────────────────────┘
                           │  Tauri IPC (invoke + emit)
┌──────────────────────────▼───────────────────────────────────┐
│                       Rust backend                           │
│  ┌─────────────┐ ┌──────────────┐ ┌────────────────────────┐ │
│  │   Scanner   │ │ Asset cache  │ │  Playtime watcher      │ │
│  │ game-       │ │ reqwest +    │ │  sysinfo polling       │ │
│  │ detector    │ │ CDN          │ │                        │ │
│  └──────┬──────┘ └──────┬───────┘ └────────────┬───────────┘ │
│         │               │                      │             │
│         └───────────────┴──────────┬───────────┘             │
│                                    │                         │
│                  ┌─────────────────▼─────────────────┐       │
│                  │       SQLite (rusqlite)           │       │
│                  │   games · play_sessions · meta    │       │
│                  └───────────────────────────────────┘       │
└──────────────────────────────────────────────────────────────┘
```

### Key Tauri commands
| Command | Purpose |
|---|---|
| `init_scan` | Runs detect → resolve → save → prefetch; emits `scan:progress` and `scan:done`. |
| `list_games` | Reads from SQLite (not from disk). Returns games with live playtime sums. |
| `launch_game` | Opens the launch URL and starts the process-based playtime watcher. Emits `playtime:updated` when the session ends. |

### On-disk layout
```
%APPDATA%\com.heartwin.gameslauncher\
├── library.db           SQLite: games, play_sessions, meta
└── assets\              Downloaded cover/hero/logo images
    ├── steam_2073850_cover.jpg
    ├── steam_2073850_hero.jpg
    ├── steam_2073850_logo.png
    └── …
```

## Tech stack
- **Frontend** — SvelteKit 2 (Svelte 5 runes), TypeScript, Vite.
- **Backend** — Rust, Tauri 2, rusqlite (bundled SQLite), reqwest (rustls-tls), sysinfo, tokio, `game-detector`, `keyvalues-parser`, `winreg`.
- **Platform** — Windows (current detection + registry/process code is Windows-only; cross-platform support would need path and process handling adjustments).

## Getting started

Prerequisites: recent **Rust** and **Node** toolchains, plus Tauri's Windows [prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
npm install
npm run tauri dev       # debug run
npm run tauri build     # release bundle
```

Useful during development:
```bash
npm run check           # svelte-check type-check
cargo check --manifest-path src-tauri/Cargo.toml
```

## Controls

| Input | Action |
|---|---|
| `←` `→` / D-pad / left stick | Move selection |
| `Enter` / `Space` / A | Launch selected game |
| `Home` / `End` | Jump to start/end of carousel |
| `1` / `2` / `3` | Filter All / Steam / Epic |
| Y button | Settings (placeholder) |

## Status

Early but functional. Known limitations:

- Windows only.
- Epic artwork depends on the game being listed on Steam too; true Epic exclusives fall back to a gradient card with the game's name.
- No settings panel yet — the cog button is a placeholder.
- GOG, Xbox/Microsoft Store, and Battle.net are not yet integrated.
- The process watcher will not track sessions when a game is launched outside this launcher (e.g. directly from Steam).
