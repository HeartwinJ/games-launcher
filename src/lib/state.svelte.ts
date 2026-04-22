import type { Game, PageName, Prefs, ScanProgress } from "./types";

/**
 * Module-level reactive state, shared across all components.
 * Components import `app` and read/write fields directly — Svelte's
 * reactivity tracks mutations on the proxy.
 */
export const app = $state({
  games: [] as Game[],
  prefs: { showSteam: true, showEpic: true } as Prefs,
  scanning: true,
  loading: true,
  progress: { stage: "Starting up", done: 0, total: 0 } as ScanProgress,
  error: null as string | null,
  page: "home" as PageName,
  menuOpen: false,
});

/** Games filtered by store-visibility prefs. */
export function visibleGames(): Game[] {
  return app.games.filter((g) => {
    if (g.source === "Steam" && !app.prefs.showSteam) return false;
    if (g.source === "Epic" && !app.prefs.showEpic) return false;
    return true;
  });
}
