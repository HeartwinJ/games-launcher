<script lang="ts">
  import { onMount } from "svelte";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import type {
    Game,
    ScanProgress,
    PlaytimeUpdate,
    Prefs,
  } from "$lib/types";
  import { app, visibleGames } from "$lib/state.svelte";
  import { navigate, initNav } from "$lib/nav";
  import { startGamepadLoop } from "$lib/gamepad";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import SettingsPage from "$lib/components/SettingsPage.svelte";
  import ActionMenu from "$lib/components/ActionMenu.svelte";

  let selected = $state(0);
  let launching = $state(false);
  let menuForGame = $state<Game | null>(null);
  let focusBeforeMenu: HTMLElement | null = null;

  // Splash dismissal state. `readyToShow` lets the main UI mount underneath
  // the splash so the "fly away" reveals real content. `splashLeaving`
  // triggers the CSS exit animations.
  let readyToShow = $state(false);
  let splashLeaving = $state(false);

  // Reset leaving state whenever a new scan begins (e.g., rescan from Settings)
  // so the fresh splash doesn't mount already-dismissing.
  $effect(() => {
    if (app.scanning) splashLeaving = false;
  });


  let cardEls: HTMLButtonElement[] = [];
  let playBtnEl: HTMLButtonElement | undefined = $state();

  const filtered = $derived(visibleGames());
  const current = $derived<Game | undefined>(filtered[selected]);

  $effect(() => {
    if (filtered.length === 0) selected = 0;
    else if (selected >= filtered.length) selected = filtered.length - 1;
  });

  async function launch(game: Game | undefined = current) {
    if (!game || launching) return;
    launching = true;
    try {
      await invoke("launch_game", { id: game.id });
    } catch (e) {
      app.error = `Failed to launch: ${e}`;
    } finally {
      setTimeout(() => (launching = false), 700);
    }
  }

  function openActionMenu() {
    if (!current) return;
    const active = document.activeElement;
    focusBeforeMenu = active instanceof HTMLElement ? active : null;
    menuForGame = current;
    app.menuOpen = true;
  }

  function closeActionMenu() {
    menuForGame = null;
    app.menuOpen = false;
    queueMicrotask(() => {
      // Restore focus to whatever opened the menu (typically the cog button).
      if (
        focusBeforeMenu &&
        document.contains(focusBeforeMenu) &&
        focusBeforeMenu instanceof HTMLElement
      ) {
        focusBeforeMenu.focus({ preventScroll: true });
      } else {
        playBtnEl?.focus({ preventScroll: true });
      }
      focusBeforeMenu = null;
    });
  }

  function onKey(e: KeyboardEvent) {
    if (app.scanning) return;
    if (e.target instanceof HTMLInputElement) return;

    // Menu open: Esc/Backspace close; arrows cycle via the generic navigator
    // (menu zone); Enter/Space let the focused button fire natively.
    if (app.menuOpen) {
      switch (e.key) {
        case "Escape":
        case "Backspace":
          e.preventDefault();
          closeActionMenu();
          return;
        case "ArrowUp":
          e.preventDefault();
          navigate("up");
          return;
        case "ArrowDown":
          e.preventDefault();
          navigate("down");
          return;
      }
      return;
    }

    // Backspace outside the menu: go back from Settings to Home.
    if (e.key === "Backspace") {
      if (app.page === "settings") {
        e.preventDefault();
        app.page = "home";
      }
      return;
    }

    switch (e.key) {
      case "ArrowLeft":
        e.preventDefault();
        navigate("left");
        break;
      case "ArrowRight":
        e.preventDefault();
        navigate("right");
        break;
      case "ArrowUp":
        e.preventDefault();
        navigate("up");
        break;
      case "ArrowDown":
        e.preventDefault();
        navigate("down");
        break;
      case "Home":
        if (app.page === "home") {
          e.preventDefault();
          cardEls[0]?.focus({ preventScroll: true });
          cardEls[0]?.scrollIntoView({ inline: "center", behavior: "smooth" });
        }
        break;
      case "End":
        if (app.page === "home") {
          e.preventDefault();
          const last = cardEls[filtered.length - 1];
          last?.focus({ preventScroll: true });
          last?.scrollIntoView({ inline: "center", behavior: "smooth" });
        }
        break;
      case "Enter":
      case " ":
        if (e.target instanceof HTMLButtonElement) break;
        e.preventDefault();
        if (app.page === "home") launch();
        break;
    }
  }

  function formatSize(bytes: number): string {
    if (!bytes) return "—";
    const gb = bytes / 1024 ** 3;
    if (gb >= 1) return `${gb.toFixed(1)} GB`;
    const mb = bytes / 1024 ** 2;
    return `${mb.toFixed(0)} MB`;
  }

  function formatPlaytime(mins: number | null | undefined): string {
    if (mins == null || mins === 0) return "—";
    if (mins < 60) return `${mins}m`;
    const totalHours = Math.floor(mins / 60);
    const m = mins % 60;
    if (totalHours < 24) {
      return m === 0 ? `${totalHours}h` : `${totalHours}h ${m}m`;
    }
    const d = Math.floor(totalHours / 24);
    const h = totalHours % 24;
    const parts: string[] = [`${d}d`];
    if (h > 0) parts.push(`${h}h`);
    if (m > 0) parts.push(`${m}m`);
    return parts.join(" ");
  }

  function initial(name: string) {
    return (name.trim()[0] ?? "?").toUpperCase();
  }

  function hueFor(id: string) {
    let h = 0;
    for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) >>> 0;
    return h % 360;
  }

  function resolveAsset(
    local: string | null,
    remote: string | null,
  ): string | null {
    if (local) {
      try {
        return convertFileSrc(local);
      } catch {
        /* noop */
      }
    }
    return remote;
  }

  async function reloadGames() {
    try {
      app.games = await invoke<Game[]>("list_games");
    } catch (e) {
      app.error = String(e);
    } finally {
      app.loading = false;
    }
  }

  async function loadPrefs() {
    try {
      const p = await invoke<Prefs>("get_prefs");
      app.prefs = p;
    } catch (e) {
      console.error("get_prefs failed:", e);
    }
  }

  onMount(() => {
    initNav();
    const unlisteners: Promise<UnlistenFn>[] = [];

    unlisteners.push(
      listen<ScanProgress>("scan:progress", (ev) => {
        app.progress = ev.payload;
      }),
    );
    unlisteners.push(
      listen<null>("scan:done", async () => {
        await reloadGames();
        // Mount main UI underneath the still-visible splash so the exit
        // animation reveals real content.
        readyToShow = true;
        // Brief beat so the 100% / "done" state registers, then fly out.
        setTimeout(() => {
          splashLeaving = true;
        }, 150);
        // After the exit animation completes, unmount the splash entirely.
        setTimeout(() => {
          app.scanning = false;
          setTimeout(() => {
            if (app.page === "home") {
              cardEls[selected]?.focus({ preventScroll: true });
            }
          }, 50);
        }, 150 + 1500);
      }),
    );
    unlisteners.push(
      listen<PlaytimeUpdate>("playtime:updated", (ev) => {
        const { gameId, playtimeMinutes } = ev.payload;
        app.games = app.games.map((g) =>
          g.id === gameId ? { ...g, playtimeMinutes } : g,
        );
      }),
    );

    loadPrefs();
    invoke("init_scan").catch((e) => {
      app.error = String(e);
      app.scanning = false;
    });

    const stopPad = startGamepadLoop({
      onDir: (dir) => {
        if (app.scanning) return;
        // navigate() is zone-aware — sidebar / menu / home / settings.
        navigate(dir);
      },
      onConfirm: () => {
        if (app.scanning) return;
        const ae = document.activeElement;
        if (ae instanceof HTMLButtonElement) {
          ae.click();
        } else if (!app.menuOpen && app.page === "home") {
          launch();
        }
      },
      onCancel: () => {
        if (app.scanning) return;
        if (app.menuOpen) {
          closeActionMenu();
        } else if (app.page === "settings") {
          app.page = "home";
        }
      },
      onSecondary: () => {
        if (app.scanning || app.menuOpen) return;
        if (app.page === "home") openActionMenu();
      },
    });

    return () => {
      stopPad();
      for (const p of unlisteners) p.then((u) => u()).catch(() => {});
    };
  });
</script>

<svelte:window on:keydown={onKey} />

{#if readyToShow}
  <Sidebar inert={app.menuOpen} />

  <main data-nav-zone={app.page} inert={app.menuOpen}>
    {#if app.page === "settings"}
      <SettingsPage />
    {:else if app.loading}
      <div class="loading">
        <div class="spinner"></div>
      </div>
    {:else if app.error && app.games.length === 0}
      <div class="empty">
        <h2>Something went wrong</h2>
        <p>{app.error}</p>
      </div>
    {:else if filtered.length === 0}
      <div class="empty">
        <h2>No games found</h2>
        <p>Install a game through Steam or the Epic Games Launcher, then relaunch.</p>
      </div>
    {:else if current}
      <!-- ========== Hero stage ========== -->
      <section class="hero" style="--hue: {hueFor(current.id)}">
        {#key current.id}
          {@const heroSrc = resolveAsset(current.heroLocal, current.heroUrl)}
          {@const coverSrc = resolveAsset(current.coverLocal, current.coverUrl)}
          {#if heroSrc}
            <img
              class="hero-bg"
              src={heroSrc}
              alt=""
              onerror={(e) =>
                ((e.currentTarget as HTMLImageElement).style.display = "none")}
            />
          {:else if coverSrc}
            <img class="hero-bg hero-bg-cover" src={coverSrc} alt="" />
          {:else}
            <div class="hero-bg hero-bg-gradient"></div>
          {/if}
        {/key}

        <div class="scrim-left"></div>
        <div class="scrim-bottom"></div>

        <div class="hero-content">
          <div class="logo-area">
            {#key current.id}
              {@const logoSrc = resolveAsset(current.logoLocal, current.logoUrl)}
              {#if logoSrc}
                <img
                  class="wordmark"
                  src={logoSrc}
                  alt={current.name}
                  onerror={(e) =>
                    ((e.currentTarget as HTMLImageElement).style.display =
                      "none")}
                />
              {/if}
              <div class="wordmark-fallback">
                <span class="wf-name">{current.name}</span>
              </div>
            {/key}
          </div>

          <!-- Glass stats capsule: [store] | [size] | [playtime] -->
          <div class="stats-capsule">
            <div class="stat store" title={current.source}>
              <span class="store-mark store-{current.source.toLowerCase()}">
                {#if current.source === "Steam"}
                  <svg viewBox="0 0 24 24" class="store-svg" aria-hidden="true">
                    <path fill="currentColor" d="M11.979 0C5.678 0 .511 4.86.022 11.037l6.432 2.658c.545-.371 1.203-.59 1.912-.59.063 0 .125.004.188.006l2.861-4.142V8.91c0-2.495 2.028-4.524 4.524-4.524 2.494 0 4.524 2.031 4.524 4.527s-2.03 4.525-4.524 4.525h-.105l-4.076 2.911c0 .052.004.105.004.159 0 1.875-1.515 3.396-3.39 3.396-1.635 0-3.016-1.173-3.331-2.727L.436 15.27C1.862 20.307 6.486 24 11.979 24c6.627 0 11.999-5.373 11.999-12S18.605 0 11.979 0zM7.54 18.21l-1.473-.61c.262.543.714.999 1.314 1.25 1.297.539 2.793-.076 3.332-1.375.263-.63.264-1.319.005-1.949s-.75-1.121-1.377-1.383c-.624-.26-1.29-.249-1.878-.03l1.523.63c.956.4 1.409 1.5 1.009 2.455-.397.957-1.497 1.41-2.454 1.012H7.54zm11.415-9.303c0-1.662-1.353-3.015-3.015-3.015-1.665 0-3.015 1.353-3.015 3.015 0 1.665 1.35 3.015 3.015 3.015 1.663 0 3.015-1.35 3.015-3.015zm-5.273-.005c0-1.252 1.013-2.266 2.265-2.266 1.249 0 2.266 1.014 2.266 2.266 0 1.251-1.017 2.265-2.266 2.265-1.253 0-2.265-1.014-2.265-2.265z" />
                  </svg>
                {:else}
                  <svg viewBox="0 0 24 24" class="store-svg" aria-hidden="true">
                    <path fill="currentColor" d="M3.537 0C2.165 0 1.66.506 1.66 1.879V18.44a4.262 4.262 0 00.02.433c.031.3.037.59.316.92.027.033.311.245.311.245.153.075.258.13.43.2l8.335 3.491c.433.199.614.276.928.27h.002c.314.006.495-.071.928-.27l8.335-3.492c.172-.07.277-.124.43-.2 0 0 .284-.211.311-.243.28-.33.285-.621.316-.92a4.261 4.261 0 00.02-.434V1.879c0-1.373-.506-1.88-1.878-1.88zm13.366 3.11h.68c1.138 0 1.688.553 1.688 1.696v1.88h-1.374v-1.8c0-.369-.17-.54-.523-.54h-.235c-.367 0-.537.17-.537.539v5.81c0 .369.17.54.537.54h.262c.353 0 .523-.171.523-.54V8.619h1.373v2.143c0 1.144-.562 1.71-1.7 1.71h-.694c-1.138 0-1.7-.566-1.7-1.71V4.82c0-1.144.562-1.709 1.7-1.709zm-12.186.08h3.114v1.274H6.117v2.603h1.648v1.275H6.117v2.774h1.74v1.275h-3.14zm3.816 0h2.198c1.138 0 1.7.564 1.7 1.708v2.445c0 1.144-.562 1.71-1.7 1.71h-.799v3.338h-1.4zm4.53 0h1.4v9.201h-1.4zm-3.13 1.235v3.392h.575c.354 0 .523-.171.523-.54V4.965c0-.368-.17-.54-.523-.54zm-1.145 5.19h8.014l-4.09 1.348z" />
                  </svg>
                {/if}
              </span>
            </div>

            <div class="divider"></div>

            <div class="stat">
              <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"
                fill="none" stroke="currentColor" stroke-width="1.7"
                stroke-linecap="round" stroke-linejoin="round">
                <ellipse cx="12" cy="5.5" rx="8" ry="2.5" />
                <path d="M4 5.5v13c0 1.4 3.6 2.5 8 2.5s8-1.1 8-2.5v-13" />
                <path d="M4 12c0 1.4 3.6 2.5 8 2.5s8-1.1 8-2.5" />
              </svg>
              <span>{formatSize(current.sizeBytes)}</span>
            </div>

            <div class="divider"></div>

            <div class="stat">
              <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"
                fill="none" stroke="currentColor" stroke-width="1.7"
                stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="9" />
                <path d="M12 7v5l3 2" />
              </svg>
              <span>{formatPlaytime(current.playtimeMinutes)}</span>
            </div>
          </div>

          <div class="actions">
            <button
              class="btn btn-play"
              class:launching
              bind:this={playBtnEl}
              onclick={() => launch()}
              disabled={launching}
            >
              <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
                <path d="M8 5v14l11-7z" fill="currentColor" />
              </svg>
              <span>{launching ? "Launching…" : "Play"}</span>
            </button>

            <button
              class="btn btn-icon"
              onclick={openActionMenu}
              aria-label="Game actions"
              aria-haspopup="dialog"
              aria-expanded={app.menuOpen}
            >
              <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"
                fill="none" stroke="currentColor" stroke-width="1.7"
                stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="3" />
                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
              </svg>
            </button>
          </div>
        </div>
      </section>

      <!-- ========== Carousel ========== -->
      <section class="carousel-wrap">
        <div class="carousel-edge left"></div>
        <div class="carousel" role="listbox" aria-label="Installed games">
          {#each filtered as game, i (game.id)}
            {@const cardSrc = resolveAsset(game.coverLocal, game.coverUrl)}
            <button
              class="card"
              style="--hue: {hueFor(game.id)}"
              bind:this={cardEls[i]}
              onfocus={() => (selected = i)}
              onclick={() => {
                selected = i;
                playBtnEl?.focus();
              }}
              onmouseenter={() => (selected = i)}
              role="option"
              aria-selected={i === selected}
              aria-label={`${game.name} on ${game.source}`}
            >
              <div class="art">
                {#if cardSrc}
                  <img
                    src={cardSrc}
                    alt=""
                    loading="lazy"
                    onerror={(e) =>
                      ((e.currentTarget as HTMLImageElement).style.display =
                        "none")}
                  />
                {/if}
                <div class="art-fallback">
                  <span>{initial(game.name)}</span>
                </div>
              </div>
              <div class="card-name">{game.name}</div>
            </button>
          {/each}
        </div>
        <div class="carousel-edge right"></div>
      </section>
    {/if}
  </main>

  {#if menuForGame}
    <ActionMenu game={menuForGame} onClose={closeActionMenu} />
  {/if}
{/if}

{#if app.scanning}
  <!-- ========== Splash ========== -->
  <div
    class="splash"
    class:splash-leaving={splashLeaving}
    role="status"
    aria-live="polite"
  >
    <div class="splash-stage">
      <div class="lockup">
        <div class="logo-disc">
          <div class="aura"></div>
          <img class="logo" src="/app-icon.svg" alt="" draggable="false" />
        </div>
      </div>

      <div class="stage-text">
        <div class="stage-label">{app.progress.stage}</div>
        {#if app.progress.total > 0}
          <div class="stage-count">
            {app.progress.done} / {app.progress.total}
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  :global(html),
  :global(body) {
    margin: 0;
    padding: 0;
    height: 100%;
    background: #07080f;
    color: #ecedf5;
    font-family:
      "Inter", ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto,
      "Helvetica Neue", Arial, sans-serif;
    font-synthesis: none;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    overflow: hidden;
  }
  :global(*) { box-sizing: border-box; }

  /* ========== Splash ========== */
  .splash {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: grid;
    place-items: center;
    background: #11121a; /* grayish-black */
    perspective: 1200px;
    /* Fade the backdrop after the icon starts its flight so the user
       sees the icon zooming past a solid plate before the reveal. */
    transition: opacity 0.75s ease 0.75s;
  }
  .splash.splash-leaving {
    opacity: 0;
    pointer-events: none;
  }

  .splash-stage {
    display: grid;
    gap: 34px;
    place-items: center;
    transform-style: preserve-3d;
  }

  .lockup {
    position: relative;
    width: 128px;
    height: 128px;
    transform-style: preserve-3d;
    animation: breathe 3.2s ease-in-out infinite;
  }
  .splash.splash-leaving .lockup {
    animation: fly 1.5s cubic-bezier(0.5, 0, 0.75, 0) forwards;
  }
  @keyframes breathe {
    0%, 100% { transform: scale(1); }
    50%      { transform: scale(1.02); }
  }
  @keyframes fly {
    0%   { transform: scale(1) translateZ(0);      opacity: 1; }
    100% { transform: scale(4) translateZ(500px);  opacity: 0; }
  }

  .logo-disc {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    overflow: hidden;
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.08),
      0 10px 28px rgba(0, 0, 0, 0.5);
  }

  /* Mostly-white conic gradient with subtle pastel hints, slow spin. */
  .aura {
    position: absolute;
    inset: 0;
    background: conic-gradient(
      from 0deg,
      #ffffff 0%,
      #ffe4ec 12%,
      #ffffff 25%,
      #e0ecff 37%,
      #ffffff 50%,
      #e6faea 62%,
      #ffffff 75%,
      #fff2dc 87%,
      #ffffff 100%
    );
    animation: auraSpin 18s linear infinite;
  }
  @keyframes auraSpin { to { transform: rotate(360deg); } }

  .logo {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    padding: 12%;
    z-index: 1;
    pointer-events: none;
    user-select: none;
    -webkit-user-drag: none;
  }

  .stage-text {
    text-align: center;
    min-height: 36px;
    transition: opacity 0.22s ease;
  }
  .splash.splash-leaving .stage-text { opacity: 0; }
  .stage-label {
    font-size: 11px;
    letter-spacing: 0.28em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.6);
    font-weight: 500;
  }
  .stage-count {
    font-size: 11.5px;
    color: rgba(255, 255, 255, 0.35);
    margin-top: 6px;
    font-variant-numeric: tabular-nums;
  }

  /* ========== Main layout ========== */
  main {
    height: 100vh;
    display: grid;
    grid-template-rows: 1fr auto;
    gap: 0;
    background: #07080f;
    /* Room for the collapsed sidebar (64px). */
    padding-left: 64px;
    overflow: hidden;
    animation: fadeIn 0.35s ease both;
  }
  @keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }

  /* Settings page breaks out of the grid rows. */
  main > :global(.page) { overflow-y: auto; }

  /* ========== Hero ========== */
  .hero {
    position: relative;
    overflow: hidden;
    min-height: 0;
  }
  .hero-bg {
    position: absolute; inset: 0;
    width: 100%; height: 100%;
    object-fit: cover; object-position: center 30%;
    animation: heroZoom 18s ease-in-out both;
  }
  .hero-bg.hero-bg-cover {
    filter: blur(40px) saturate(130%);
    transform: scale(1.3); opacity: 0.75;
  }
  .hero-bg-gradient {
    background:
      radial-gradient(70% 80% at 30% 20%, hsl(var(--hue) 60% 45% / 0.55), transparent 60%),
      radial-gradient(70% 70% at 85% 80%, hsl(calc(var(--hue) + 60) 60% 40% / 0.5), transparent 65%),
      #0b0d17;
  }
  @keyframes heroZoom {
    from { transform: scale(1.02); }
    to { transform: scale(1.08); }
  }

  .scrim-left {
    position: absolute; inset: 0;
    background: linear-gradient(
      90deg,
      rgba(7, 8, 15, 0.92) 0%,
      rgba(7, 8, 15, 0.82) 18%,
      rgba(7, 8, 15, 0.55) 40%,
      rgba(7, 8, 15, 0.2) 62%,
      rgba(7, 8, 15, 0) 85%
    );
  }
  .scrim-bottom {
    position: absolute; inset: 0;
    background: linear-gradient(
      180deg,
      rgba(7, 8, 15, 0) 55%,
      rgba(7, 8, 15, 0.55) 85%,
      rgba(7, 8, 15, 0.95) 100%
    );
  }

  .hero-content {
    position: relative;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    gap: 20px;
    padding: 32px 40px 40px;
    max-width: min(640px, 55%);
  }

  .logo-area {
    position: relative;
    height: clamp(90px, 14vh, 150px);
    display: flex;
    align-items: flex-end;
    animation: rise 0.4s ease both;
  }
  .wordmark {
    max-height: 100%;
    max-width: 100%;
    object-fit: contain;
    object-position: left bottom;
    filter: drop-shadow(0 6px 22px rgba(0, 0, 0, 0.6));
  }
  .wordmark-fallback { position: absolute; left: 0; bottom: 0; }
  .wordmark + .wordmark-fallback { display: none; }
  .wf-name {
    display: inline-block;
    font-size: clamp(32px, 4.5vw, 56px);
    font-weight: 800;
    line-height: 1.02;
    letter-spacing: -0.02em;
    background: linear-gradient(135deg, #ffffff, #cfcbff);
    -webkit-background-clip: text; background-clip: text; color: transparent;
    text-shadow: 0 6px 30px rgba(0, 0, 0, 0.5);
  }
  @keyframes rise {
    from { opacity: 0; transform: translateY(8px); }
    to { opacity: 1; transform: translateY(0); }
  }

  /* ========== Stats capsule ========== */
  .stats-capsule {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 14px;
    padding: 6px 16px 6px 6px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.05);
    backdrop-filter: blur(22px) saturate(160%);
    -webkit-backdrop-filter: blur(22px) saturate(160%);
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.08),
      0 8px 24px rgba(0, 0, 0, 0.25);
    color: rgba(236, 237, 245, 0.88);
    animation: rise 0.45s ease both;
    animation-delay: 30ms;
  }
  .stat {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    font-weight: 500;
    white-space: nowrap;
  }
  .stat .icon {
    width: 15px; height: 15px;
    color: rgba(236, 237, 245, 0.65);
  }
  .divider {
    width: 1px; height: 16px;
    background: rgba(255, 255, 255, 0.12);
  }

  .store-mark {
    display: inline-grid;
    place-items: center;
    width: 26px; height: 26px;
    border-radius: 50%;
    overflow: hidden;
    color: #fff;
  }
  .store-mark .store-svg { width: 16px; height: 16px; }
  .store-mark.store-steam {
    background: #171d25;
    border: 1px solid rgba(255, 255, 255, 0.14);
  }
  .store-mark.store-epic {
    background: #2a2a2a;
    border: 1px solid rgba(255, 255, 255, 0.18);
  }

  /* ========== Glass buttons ========== */
  .actions {
    display: flex;
    gap: 10px;
    animation: rise 0.5s ease both;
    animation-delay: 60ms;
  }
  .btn {
    all: unset;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    height: 52px;
    border-radius: 14px;
    font-weight: 600;
    font-size: 14.5px;
    color: rgba(255, 255, 255, 0.95);
    cursor: pointer;
    background: linear-gradient(
      180deg,
      rgba(255, 255, 255, 0.1),
      rgba(255, 255, 255, 0.04)
    );
    backdrop-filter: blur(22px) saturate(160%);
    -webkit-backdrop-filter: blur(22px) saturate(160%);
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.12),
      0 10px 28px rgba(0, 0, 0, 0.28);
    transition: background 0.18s ease, border-color 0.18s ease, transform 0.15s ease;
  }
  .btn:hover {
    background: linear-gradient(
      180deg,
      rgba(255, 255, 255, 0.16),
      rgba(255, 255, 255, 0.08)
    );
    border-color: rgba(255, 255, 255, 0.22);
    transform: translateY(-1px);
  }
  .btn:active { transform: translateY(0) scale(0.99); }
  .btn:focus-visible {
    outline: none;
    border-color: rgba(255, 255, 255, 0.55);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.18),
      0 0 0 3px rgba(255, 255, 255, 0.1),
      0 10px 28px rgba(0, 0, 0, 0.28);
  }
  .btn:disabled { opacity: 0.75; cursor: default; transform: none; }

  .btn-play { min-width: 220px; padding: 0 28px; }
  .btn-play.launching { animation: launchPulse 0.6s ease; }
  @keyframes launchPulse {
    0% { filter: brightness(1); }
    50% { filter: brightness(1.35); }
    100% { filter: brightness(1); }
  }

  .btn-icon { width: 52px; padding: 0; }
  .cog-wrap { position: relative; }

  /* ========== Carousel ========== */
  .carousel-wrap {
    position: relative;
    background: linear-gradient(180deg, rgba(7, 8, 15, 0.6), #07080f 40%);
    padding: 6px 0 14px;
  }
  .carousel {
    display: flex;
    gap: 14px;
    padding: 32px 24px 16px;
    overflow-x: auto;
    overflow-y: hidden;
    scroll-padding-inline: 40%;
    scroll-behavior: smooth;
    scrollbar-width: none;
  }
  .carousel::-webkit-scrollbar { display: none; }
  .carousel-edge {
    position: absolute;
    top: 0; bottom: 0;
    width: 60px;
    pointer-events: none;
    z-index: 2;
  }
  .carousel-edge.left {
    left: 0;
    background: linear-gradient(to right, #07080f 10%, transparent);
  }
  .carousel-edge.right {
    right: 0;
    background: linear-gradient(to left, #07080f 10%, transparent);
  }

  .card {
    all: unset;
    flex: 0 0 auto;
    width: 150px;
    cursor: pointer;
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 4px;
    border-radius: 14px;
    transition:
      transform 0.22s cubic-bezier(0.2, 0.8, 0.2, 1),
      box-shadow 0.22s ease;
    transform-origin: center center;
  }
  .card:focus { outline: none; }
  .card:focus-visible { outline: none; }

  .card .art {
    position: relative;
    aspect-ratio: 600 / 900;
    border-radius: 10px;
    overflow: hidden;
    background: linear-gradient(
      160deg,
      hsl(var(--hue, 250) 40% 20%),
      hsl(calc(var(--hue, 250) + 50) 40% 12%)
    );
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.06);
    transition: box-shadow 0.22s ease;
  }
  .card .art img {
    position: absolute; inset: 0;
    width: 100%; height: 100%;
    object-fit: cover;
  }
  .card .art-fallback {
    position: absolute; inset: 0;
    display: grid; place-items: center;
    font-size: 50px; font-weight: 800;
    color: rgba(255, 255, 255, 0.85);
    text-shadow: 0 6px 24px rgba(0, 0, 0, 0.5);
    background: radial-gradient(
      circle at 30% 20%,
      hsl(var(--hue, 250) 60% 45% / 0.45),
      transparent 60%
    );
  }
  .card .art img + .art-fallback { display: none; }

  .card-name {
    font-size: 12px;
    font-weight: 500;
    color: rgba(236, 237, 245, 0.7);
    text-align: center;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    padding: 0 4px;
    transition: color 0.2s ease;
  }

  /* Only the focused card is highlighted — TV-style spatial focus. */
  .card:focus { transform: translateY(-8px) scale(1.06); }
  .card:focus .art {
    box-shadow:
      0 0 0 2px rgba(255, 255, 255, 0.95),
      0 0 0 5px rgba(255, 255, 255, 0.08),
      0 22px 50px rgba(0, 0, 0, 0.55),
      0 0 30px rgba(255, 255, 255, 0.08);
  }
  .card:focus .card-name { color: #fff; }

  /* ========== Loading / empty ========== */
  .loading { display: grid; place-items: center; height: 100%; }
  .spinner {
    width: 42px; height: 42px; border-radius: 50%;
    border: 3px solid rgba(255, 255, 255, 0.08);
    border-top-color: rgba(255, 255, 255, 0.8);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .empty {
    display: grid; place-items: center;
    height: 100%;
    text-align: center;
    color: rgba(236, 237, 245, 0.7);
    padding: 24px;
  }
  .empty h2 { font-size: 22px; font-weight: 600; margin: 0 0 6px; }
  .empty p { margin: 0; max-width: 420px; color: rgba(236, 237, 245, 0.55); }
</style>
