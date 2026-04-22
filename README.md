# Games Launcher

A minimal, controller-friendly Windows game launcher that aggregates your installed Steam and Epic Games library into a single TV-style grid. Built with Tauri 2, SvelteKit, and SQLite.

No account sign-in, no storefront, no clutter. Just your installed games, beautiful artwork, and the shortest possible path from launch to play — designed to work equally well with a keyboard, mouse, or Xbox controller.

## Features

### Library
- **Unified detection** — scans Steam (via local `.acf` manifests) and Epic Games Launcher (via its `.item` manifests) through the [`game-detector`](https://crates.io/crates/game-detector) crate.
- **Artwork for everything** — Steam games pull cover, landscape hero, and transparent wordmark straight from Steam's CDN. Epic games are matched against Steam Community's public search by normalized title and reuse the same CDN, so multiplatform titles (GTA V, Cyberpunk, The Finals, etc.) get the full art treatment. True Epic exclusives fall back to a stylized gradient tile with the game name.
- **Utility filtering** — Steamworks Redistributables, Proton variants, Steam Linux Runtime, and SteamVR are excluded automatically.
- **Local SQLite library** — games, remote + local asset paths, timestamps, and hidden/removed flags live in `library.db` under the app's data directory. Games that disappear between scans are soft-deleted, preserving their playtime history in case they return.
- **Asset prefetch** — every cover / hero / logo is downloaded into `assets/` with a concurrency cap of 6, then served locally via Tauri's asset protocol. Offline launches are instant.

### Playtime tracking
- Native, process-based session tracking via [`sysinfo`](https://crates.io/crates/sysinfo). After you click Play, a watcher polls every 5s for any process whose executable path lives inside the game's install directory. A session opens the moment one appears and closes 20s after the last one exits — a grace window absorbs anti-cheat restarts.
- Sessions are stored in `play_sessions`; the displayed playtime is a live `SUM(duration_seconds)` from that table.
- **First-run backfill** — on the initial scan the launcher imports existing Steam playtime from `localconfig.vdf` as a single synthetic session per game, so nothing is lost.
- **Reset-per-game** from the action menu deletes every session for that game.
- Orphaned sessions (launcher killed mid-session) are zeroed on the next startup.
- Playtime format tiers: `42m` / `4h 32m` / `1d 3h 15m`.

### Interface
- **Splash screen** with your app logo in a circular disc, a slow-rotating mostly-white pastel aura behind, and a perspective-based fly-out reveal (scale + `translateZ` + opacity over 1.5s) when the scan completes. Main UI cross-fades in underneath.
- **Sidebar** (64px collapsed, 220px on hover / keyboard focus): Home and Settings in the vertical center, Exit pinned bottom (hidden while collapsed). Icons centered in the collapsed state.
- **Hero + carousel** home page: full-bleed hero image (or blurred portrait fallback) for the focused game on top, horizontal portrait card strip below. Ambient blur of the hero serves as the viewport backdrop and cross-fades on selection change.
- **Per-game action modal**: Open install folder · Copy install path · Hide from library · Reset playtime · Uninstall. Opens from the cog button next to Play; centered, glass-themed, focus-locked.
- **Settings page**: store visibility (Steam / Epic), fullscreen toggle (on by default, applies live and persists for next launch), rescan library, open data folder, and an About section.
- **Glassmorphic theme** throughout — frosted panels, top-lit gradient borders, consistent across buttons, capsules, sidebar, and modals.
- **Real Steam + Epic brand logos** for store marks (Simple Icons path data).

### TV-style spatial navigation
- Zoned focus system via `data-nav-zone="..."` attributes on container elements.
- **Vertical nav is zone-constrained** — inside the sidebar, up/down cycle through sidebar items only. Inside home or settings, up/down stay within that zone's buttons.
- **Left edge opens the sidebar** — pressing Left when there's no in-zone candidate (e.g. from the Play button or the first carousel card) focuses the sidebar's active page item.
- **Sidebar Right restores the last main focus** (memory navigation) — so bouncing out and back lands you exactly where you were.
- **Spatial scoring with a directional cone** — a candidate must be more in the requested direction than off-axis, so "slightly off to the side and way up" doesn't win as "left".
- **Modal locks focus** — when the action menu or any future dialog is open, `inert` on the rest of the UI removes everything outside the modal from tab order and interactive state.
- **Only the focused element is highlighted** — the card "ring" is driven by `:focus`, not a persistent selected-class, so exactly one thing at a time has the TV-style highlight.

### Launching
- Steam: `steam://rungameid/<appid>`.
- Epic: `com.epicgames.launcher://apps/<app_name>?action=launch&silent=true`.
- Uninstall deep-links: Steam `steam://uninstall/<appid>`, Epic `com.epicgames.launcher://apps/<app_name>?action=uninstall`.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                         SvelteKit UI                         │
│  Splash · Sidebar · Home (Hero+Carousel) · Settings · Modal  │
│           TV-style spatial navigator · Gamepad loop          │
└──────────────────────────┬───────────────────────────────────┘
                           │  Tauri IPC (invoke + events)
┌──────────────────────────▼───────────────────────────────────┐
│                       Rust backend                           │
│  ┌─────────────┐ ┌──────────────┐ ┌────────────────────────┐ │
│  │   Scanner   │ │ Asset cache  │ │  Playtime watcher      │ │
│  │ game-       │ │ reqwest +    │ │  sysinfo polling       │ │
│  │ detector    │ │ CDN          │ │  + DB sessions         │ │
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

### Tauri commands
| Command | Purpose |
|---|---|
| `init_scan` | Runs detect → resolve → save → prefetch; emits `scan:progress` and `scan:done` events. |
| `list_games` | Reads from SQLite (not from disk). Returns games with live playtime sums, hidden/removed entries filtered out. |
| `launch_game` | Opens the launch URL and starts the process-based playtime watcher. Emits `playtime:updated` when the session ends. |
| `hide_game` | Soft-hides a game (`hidden=1`) so it drops out of `list_games`. |
| `reset_playtime` | Deletes every session for a game_id and emits `playtime:updated` with 0. |
| `uninstall_game` | Deep-links the store's uninstall URL scheme for the game. |
| `open_data_folder` | Opens the app's data directory in the OS file manager. |
| `get_prefs` / `set_pref` | Reads / writes persistent user prefs (store visibility) via the `meta` table. |
| `set_fullscreen` | Applies the window's fullscreen state live and persists the pref in one call. |

### On-disk layout
```
%APPDATA%\com.heartwin.gameslauncher\
├── library.db           SQLite: games, play_sessions, meta
└── assets\              Downloaded cover / hero / logo images
    ├── steam_2073850_cover.jpg
    ├── steam_2073850_hero.jpg
    ├── steam_2073850_logo.png
    └── …
```

## Tech stack
- **Frontend** — SvelteKit 2 (Svelte 5 runes), TypeScript, Vite.
- **Backend** — Rust, Tauri 2, rusqlite (bundled SQLite), reqwest (rustls-tls), sysinfo, tokio, `game-detector`, `keyvalues-parser`, `winreg`, `urlencoding`.
- **Platform** — Windows. Detection, registry reads, and the process watcher are all Windows-specific; cross-platform work would require parallel implementations.

## Getting started

Prerequisites: recent **Rust** and **Node** toolchains plus Tauri's [Windows prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
npm install
npm run tauri dev       # debug run
npm run tauri build     # release bundle
```

Useful during development:
```bash
npm run check                                       # svelte-check type check
cargo check --manifest-path src-tauri/Cargo.toml    # rust type check
```

Regenerating the app icons after swapping `static/app-icon.svg`:
```bash
npx tauri icon static/app-icon.svg
```
This rewrites `icon.ico`, `icon.icns`, and the standard PNG set in `src-tauri/icons/`. If you intend to Windows-Store package the app you'll additionally need the `SquareXxXLogo.png` tile sizes, but by default they aren't used.

After replacing the ICO you'll need to **stop all running dev instances** and `cargo clean -p games-launcher` once so cargo re-runs the build script and the linker re-embeds the icon into the exe's resource table — icons are baked in at compile time, not loaded at runtime.

## Controls

| Input | Action |
|---|---|
| `←` `→` / D-pad / left stick | Navigate within the current zone (carousel, sidebar items, settings rows, modal items) |
| `↑` `↓` | Same, zone-constrained |
| Left from first card / Play button / settings leftmost | Open the sidebar on the active page |
| Right from sidebar | Return to the last focused main element |
| `Enter` / `Space` / A | Activate the focused button |
| Y button | Open the per-game action menu |
| `Esc` / `Backspace` / B | Close the action menu if open, else Settings → Home |
| `Home` / `End` | Jump to first / last carousel card |
| `1` / `2` / `3` | Filter All / Steam / Epic (keyboard shortcut) |

## Known limitations
- **Windows only.** Detection and playtime tracking are Windows-specific.
- **Epic exclusives show a gradient fallback** — artwork relies on the game also existing on Steam's catalog. GOG, Xbox / Microsoft Store, and Battle.net are not yet integrated.
- **Playtime only tracks sessions launched through this launcher.** Games started directly from Steam / Epic won't be tracked (beyond the one-time Steam backfill).
- **The hint bar** (context-sensitive button-glyph footer) is planned but not yet implemented.
