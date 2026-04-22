# Games Launcher

A minimal, controller-friendly Windows game launcher that aggregates your installed Steam and Epic Games library into a single TV-style grid.

No account sign-in, no storefront, no clutter. Just your installed games, beautiful artwork, and the shortest possible path from launch to play — designed to work equally well with a keyboard, mouse, or Xbox controller.

## Features

### Library
- **Unified detection** — automatically finds your installed Steam and Epic Games.
- **Artwork for everything** — cover, landscape hero, and transparent wordmark artwork for every detected game. Epic-only titles fall back to a stylized gradient tile with the game name.
- **Utility filtering** — runtimes, redistributables, and compatibility layers are excluded automatically.
- **Offline-ready** — artwork is cached locally so launches are instant.

### Playtime tracking
- Sessions are tracked automatically when you launch a game through the launcher.
- On first run, existing Steam playtime is imported so nothing is lost.
- Reset playtime per game from the action menu.
- Playtime is shown as `42m` / `4h 32m` / `1d 3h 15m`.

### Interface
- **Splash screen** with a smooth transition into the library.
- **Sidebar** with Home, Settings, and Exit.
- **Hero + carousel** home page: full-bleed hero image for the focused game on top, horizontal portrait card strip below.
- **Per-game action menu**: Open install folder · Copy install path · Hide from library · Reset playtime · Uninstall.
- **Settings**: store visibility (Steam / Epic), fullscreen toggle, rescan library, open data folder.
- **Glassmorphic theme** — frosted panels and consistent styling throughout.

### TV-style navigation
- Fully navigable with a gamepad, arrow keys, or mouse.
- Smart spatial focus so the highlight always lands where you expect.
- The sidebar remembers your previous spot when you return to it.

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
- **Windows only.**
- **Epic exclusives show a gradient fallback** — full artwork isn't available for Epic-only titles. GOG, Xbox / Microsoft Store, and Battle.net are not yet supported.
- **Playtime only tracks sessions launched through this launcher.** Games started directly from Steam / Epic won't be tracked (beyond the one-time Steam backfill).
