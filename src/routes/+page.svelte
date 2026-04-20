<script lang="ts">
  import { onMount } from "svelte";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import type {
    Game,
    GameSource,
    ScanProgress,
    PlaytimeUpdate,
  } from "$lib/types";
  import { startGamepadLoop } from "$lib/gamepad";

  type Filter = "All" | GameSource;

  // --- Splash / scan state ---
  let scanning = $state(true);
  let progress = $state<ScanProgress>({
    stage: "Starting up",
    done: 0,
    total: 0,
  });

  // --- Library state ---
  let games = $state<Game[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let filter = $state<Filter>("All");
  let selected = $state(0);
  let launching = $state(false);

  let cardEls: HTMLButtonElement[] = [];

  const filtered = $derived(
    filter === "All" ? games : games.filter((g) => g.source === filter),
  );
  const current = $derived<Game | undefined>(filtered[selected]);

  $effect(() => {
    if (filtered.length === 0) selected = 0;
    else if (selected >= filtered.length) selected = filtered.length - 1;
  });

  function move(delta: number) {
    if (filtered.length === 0) return;
    let next = selected + delta;
    if (next < 0) next = 0;
    if (next >= filtered.length) next = filtered.length - 1;
    if (next === selected) return;
    selected = next;
    queueMicrotask(() => {
      cardEls[selected]?.scrollIntoView({
        block: "nearest",
        inline: "center",
        behavior: "smooth",
      });
    });
  }

  async function launch(game: Game | undefined = current) {
    if (!game || launching) return;
    launching = true;
    try {
      await invoke("launch_game", { id: game.id });
    } catch (e) {
      error = `Failed to launch: ${e}`;
    } finally {
      setTimeout(() => (launching = false), 700);
    }
  }

  function openSettings() {
    console.log("settings: coming soon");
  }

  function onKey(e: KeyboardEvent) {
    if (scanning) return;
    if (e.target instanceof HTMLInputElement) return;
    switch (e.key) {
      case "ArrowLeft":
        e.preventDefault();
        move(-1);
        break;
      case "ArrowRight":
        e.preventDefault();
        move(1);
        break;
      case "Home":
        e.preventDefault();
        selected = 0;
        cardEls[0]?.scrollIntoView({ inline: "center", behavior: "smooth" });
        break;
      case "End":
        e.preventDefault();
        selected = Math.max(0, filtered.length - 1);
        cardEls[selected]?.scrollIntoView({ inline: "center", behavior: "smooth" });
        break;
      case "Enter":
      case " ":
        e.preventDefault();
        launch();
        break;
      case "1":
        filter = "All";
        break;
      case "2":
        filter = "Steam";
        break;
      case "3":
        filter = "Epic";
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
    const h = Math.floor(mins / 60);
    const m = mins % 60;
    return m === 0 ? `${h}h` : `${h}h ${m}m`;
  }

  function initial(name: string) {
    return (name.trim()[0] ?? "?").toUpperCase();
  }

  function hueFor(id: string) {
    let h = 0;
    for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) >>> 0;
    return h % 360;
  }

  // Prefer the locally-cached file, fall back to remote URL, else null.
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
      games = await invoke<Game[]>("list_games");
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    const unlisteners: Promise<UnlistenFn>[] = [];

    // Scan progress
    unlisteners.push(
      listen<ScanProgress>("scan:progress", (ev) => {
        progress = ev.payload;
      }),
    );
    // Scan finished → read DB, reveal UI
    unlisteners.push(
      listen<null>("scan:done", async () => {
        await reloadGames();
        // Brief pause so the splash's "done" state is seen, then fade out.
        setTimeout(() => {
          scanning = false;
        }, 250);
      }),
    );
    // Playtime updates from process watcher
    unlisteners.push(
      listen<PlaytimeUpdate>("playtime:updated", (ev) => {
        const { gameId, playtimeMinutes } = ev.payload;
        games = games.map((g) =>
          g.id === gameId ? { ...g, playtimeMinutes } : g,
        );
      }),
    );

    // Kick off the scan
    invoke("init_scan").catch((e) => {
      error = String(e);
      scanning = false;
    });

    // Gamepad
    const stopPad = startGamepadLoop({
      onDir: (dir) => {
        if (scanning) return;
        if (dir === "left") move(-1);
        else if (dir === "right") move(1);
      },
      onConfirm: () => {
        if (!scanning) launch();
      },
      onSecondary: () => {
        if (!scanning) openSettings();
      },
    });

    return () => {
      stopPad();
      for (const p of unlisteners) p.then((u) => u()).catch(() => {});
    };
  });
</script>

<svelte:window on:keydown={onKey} />

{#if scanning}
  <!-- ========== Splash ========== -->
  <div class="splash" role="status" aria-live="polite">
    <div class="splash-backdrop" aria-hidden="true">
      <div class="splash-blob a"></div>
      <div class="splash-blob b"></div>
      <div class="splash-blob c"></div>
    </div>

    <div class="splash-panel">
      <div class="splash-brand">
        <div class="splash-mark"></div>
        <div class="splash-title">Games</div>
        <div class="splash-subtitle">Launcher</div>
      </div>

      <div class="splash-status">
        <div class="splash-spinner"></div>
        <div class="splash-stage">{progress.stage}</div>
        {#if progress.total > 0}
          <div class="splash-count">
            {progress.done} / {progress.total}
          </div>
          <div class="splash-bar">
            <div
              class="splash-bar-fill"
              style="width: {Math.min(100, (progress.done / progress.total) * 100)}%"
            ></div>
          </div>
        {/if}
      </div>
    </div>
  </div>
{:else}
  <main>
    {#if loading}
      <div class="loading">
        <div class="spinner"></div>
      </div>
    {:else if error && games.length === 0}
      <div class="empty">
        <h2>Something went wrong</h2>
        <p>{error}</p>
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

          <!-- Minimal glass stats capsule: [store] | [size] | [playtime] -->
          <div class="stats-capsule">
            <div class="stat store" title={current.source}>
              <span class="store-mark store-{current.source.toLowerCase()}">
                {#if current.source === "Steam"}
                  <svg viewBox="0 0 24 24" class="store-svg" aria-hidden="true">
                    <path
                      fill="currentColor"
                      d="M11.979 0C5.678 0 .511 4.86.022 11.037l6.432 2.658c.545-.371 1.203-.59 1.912-.59.063 0 .125.004.188.006l2.861-4.142V8.91c0-2.495 2.028-4.524 4.524-4.524 2.494 0 4.524 2.031 4.524 4.527s-2.03 4.525-4.524 4.525h-.105l-4.076 2.911c0 .052.004.105.004.159 0 1.875-1.515 3.396-3.39 3.396-1.635 0-3.016-1.173-3.331-2.727L.436 15.27C1.862 20.307 6.486 24 11.979 24c6.627 0 11.999-5.373 11.999-12S18.605 0 11.979 0zM7.54 18.21l-1.473-.61c.262.543.714.999 1.314 1.25 1.297.539 2.793-.076 3.332-1.375.263-.63.264-1.319.005-1.949s-.75-1.121-1.377-1.383c-.624-.26-1.29-.249-1.878-.03l1.523.63c.956.4 1.409 1.5 1.009 2.455-.397.957-1.497 1.41-2.454 1.012H7.54zm11.415-9.303c0-1.662-1.353-3.015-3.015-3.015-1.665 0-3.015 1.353-3.015 3.015 0 1.665 1.35 3.015 3.015 3.015 1.663 0 3.015-1.35 3.015-3.015zm-5.273-.005c0-1.252 1.013-2.266 2.265-2.266 1.249 0 2.266 1.014 2.266 2.266 0 1.251-1.017 2.265-2.266 2.265-1.253 0-2.265-1.014-2.265-2.265z"
                    />
                  </svg>
                {:else}
                  <svg viewBox="0 0 24 24" class="store-svg" aria-hidden="true">
                    <path
                      fill="currentColor"
                      d="M3.537 0C2.165 0 1.66.506 1.66 1.879V18.44a4.262 4.262 0 00.02.433c.031.3.037.59.316.92.027.033.311.245.311.245.153.075.258.13.43.2l8.335 3.491c.433.199.614.276.928.27h.002c.314.006.495-.071.928-.27l8.335-3.492c.172-.07.277-.124.43-.2 0 0 .284-.211.311-.243.28-.33.285-.621.316-.92a4.261 4.261 0 00.02-.434V1.879c0-1.373-.506-1.88-1.878-1.88zm13.366 3.11h.68c1.138 0 1.688.553 1.688 1.696v1.88h-1.374v-1.8c0-.369-.17-.54-.523-.54h-.235c-.367 0-.537.17-.537.539v5.81c0 .369.17.54.537.54h.262c.353 0 .523-.171.523-.54V8.619h1.373v2.143c0 1.144-.562 1.71-1.7 1.71h-.694c-1.138 0-1.7-.566-1.7-1.71V4.82c0-1.144.562-1.709 1.7-1.709zm-12.186.08h3.114v1.274H6.117v2.603h1.648v1.275H6.117v2.774h1.74v1.275h-3.14zm3.816 0h2.198c1.138 0 1.7.564 1.7 1.708v2.445c0 1.144-.562 1.71-1.7 1.71h-.799v3.338h-1.4zm4.53 0h1.4v9.201h-1.4zm-3.13 1.235v3.392h.575c.354 0 .523-.171.523-.54V4.965c0-.368-.17-.54-.523-.54zm-3.74 10.147a1.708 1.708 0 01.591.108 1.745 1.745 0 01.49.299l-.452.546a1.247 1.247 0 00-.308-.195.91.91 0 00-.363-.068.658.658 0 00-.28.06.703.703 0 00-.224.163.783.783 0 00-.151.243.799.799 0 00-.056.299v.008a.852.852 0 00.056.31.7.7 0 00.157.245.736.736 0 00.238.16.774.774 0 00.303.058.79.79 0 00.445-.116v-.339h-.548v-.565H7.37v1.255a2.019 2.019 0 01-.524.307 1.789 1.789 0 01-.683.123 1.642 1.642 0 01-.602-.107 1.46 1.46 0 01-.478-.3 1.371 1.371 0 01-.318-.455 1.438 1.438 0 01-.115-.58v-.008a1.426 1.426 0 01.113-.57 1.449 1.449 0 01.312-.46 1.418 1.418 0 01.474-.309 1.58 1.58 0 01.598-.111 1.708 1.708 0 01.045 0zm11.963.008a2.006 2.006 0 01.612.094 1.61 1.61 0 01.507.277l-.386.546a1.562 1.562 0 00-.39-.205 1.178 1.178 0 00-.388-.07.347.347 0 00-.208.052.154.154 0 00-.07.127v.008a.158.158 0 00.022.084.198.198 0 00.076.066.831.831 0 00.147.06c.062.02.14.04.236.061a3.389 3.389 0 01.43.122 1.292 1.292 0 01.328.17.678.678 0 01.207.24.739.739 0 01.071.337v.008a.865.865 0 01-.081.382.82.82 0 01-.229.285 1.032 1.032 0 01-.353.18 1.606 1.606 0 01-.46.061 2.16 2.16 0 01-.71-.116 1.718 1.718 0 01-.593-.346l.43-.514c.277.223.578.335.9.335a.457.457 0 00.236-.05.157.157 0 00.082-.142v-.008a.15.15 0 00-.02-.077.204.204 0 00-.073-.066.753.753 0 00-.143-.062 2.45 2.45 0 00-.233-.062 5.036 5.036 0 01-.413-.113 1.26 1.26 0 01-.331-.16.72.72 0 01-.222-.243.73.73 0 01-.082-.36v-.008a.863.863 0 01.074-.359.794.794 0 01.214-.283 1.007 1.007 0 01.34-.185 1.423 1.423 0 01.448-.066 2.006 2.006 0 01.025 0zm-9.358.025h.742l1.183 2.81h-.825l-.203-.499H8.623l-.198.498h-.81zm2.197.02h.814l.663 1.08.663-1.08h.814v2.79h-.766v-1.602l-.711 1.091h-.016l-.707-1.083v1.593h-.754zm3.469 0h2.235v.658h-1.473v.422h1.334v.61h-1.334v.442h1.493v.658h-2.255zm-5.3.897l-.315.793h.624zm-1.145 5.19h8.014l-4.09 1.348z"
                    />
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
              onclick={openSettings}
              title="Settings (coming soon)"
              aria-label="Settings"
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
              class:selected={i === selected}
              style="--hue: {hueFor(game.id)}"
              bind:this={cardEls[i]}
              onclick={() => {
                if (selected === i) launch(game);
                else selected = i;
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
    background: #07080f;
    animation: fadeOutLeave 0.35s ease forwards;
    animation-play-state: paused;
  }
  .splash-backdrop {
    position: absolute;
    inset: 0;
    overflow: hidden;
    z-index: 0;
    opacity: 0.5;
    filter: blur(80px);
  }
  .splash-blob {
    position: absolute;
    width: 55vw;
    height: 55vw;
    border-radius: 50%;
    mix-blend-mode: screen;
  }
  .splash-blob.a {
    background: radial-gradient(circle, #8b7bff 0%, transparent 65%);
    left: -10vw; top: -20vw;
    animation: drift-a 22s ease-in-out infinite alternate;
  }
  .splash-blob.b {
    background: radial-gradient(circle, #1dd3da 0%, transparent 65%);
    right: -10vw; top: 30vh;
    animation: drift-b 26s ease-in-out infinite alternate;
  }
  .splash-blob.c {
    background: radial-gradient(circle, #ff6ec7 0%, transparent 60%);
    left: 25vw; bottom: -20vw;
    animation: drift-c 30s ease-in-out infinite alternate;
  }
  @keyframes drift-a {
    to { transform: translate(6vw, 4vh) scale(1.1); }
  }
  @keyframes drift-b {
    to { transform: translate(-5vw, -3vh) scale(1.08); }
  }
  @keyframes drift-c {
    to { transform: translate(-4vw, -5vh) scale(1.12); }
  }

  .splash-panel {
    position: relative;
    z-index: 1;
    display: grid;
    gap: 34px;
    padding: 40px 48px;
    border-radius: 24px;
    background: rgba(255, 255, 255, 0.04);
    backdrop-filter: blur(24px) saturate(160%);
    -webkit-backdrop-filter: blur(24px) saturate(160%);
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.1),
      0 30px 60px rgba(0, 0, 0, 0.5);
    min-width: 420px;
  }
  .splash-brand {
    display: flex;
    align-items: baseline;
    gap: 14px;
  }
  .splash-mark {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: linear-gradient(135deg, #8b7bff, #1dd3da);
    box-shadow: 0 0 22px rgba(139, 123, 255, 0.7);
    transform: translateY(3px);
  }
  .splash-title {
    font-size: 28px;
    font-weight: 700;
    letter-spacing: 0.02em;
    background: linear-gradient(135deg, #fff, #c9c4ff);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  .splash-subtitle {
    font-size: 13px;
    letter-spacing: 0.35em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.5);
  }
  .splash-status {
    display: grid;
    gap: 12px;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    row-gap: 14px;
  }
  .splash-spinner {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.1);
    border-top-color: rgba(255, 255, 255, 0.85);
    animation: spin 0.9s linear infinite;
  }
  .splash-stage {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.85);
  }
  .splash-count {
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: rgba(255, 255, 255, 0.55);
  }
  .splash-bar {
    grid-column: 1 / -1;
    height: 3px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.07);
    overflow: hidden;
  }
  .splash-bar-fill {
    height: 100%;
    background: linear-gradient(90deg, #8b7bff, #1dd3da);
    border-radius: inherit;
    transition: width 0.3s cubic-bezier(0.2, 0.8, 0.2, 1);
    box-shadow: 0 0 16px rgba(139, 123, 255, 0.5);
  }

  /* ========== Main layout ========== */
  main {
    height: 100vh;
    display: grid;
    grid-template-rows: 1fr auto;
    gap: 0;
    background: #07080f;
    animation: fadeIn 0.35s ease both;
  }
  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  /* ========== Hero stage ========== */
  .hero {
    position: relative;
    overflow: hidden;
    min-height: 0;
  }
  .hero-bg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center 30%;
    animation: heroZoom 18s ease-in-out both;
  }
  .hero-bg.hero-bg-cover {
    filter: blur(40px) saturate(130%);
    transform: scale(1.3);
    opacity: 0.75;
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
    position: absolute;
    inset: 0;
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
    position: absolute;
    inset: 0;
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
  .wordmark-fallback {
    position: absolute;
    left: 0;
    bottom: 0;
  }
  .wordmark + .wordmark-fallback { display: none; }
  .wf-name {
    display: inline-block;
    font-size: clamp(32px, 4.5vw, 56px);
    font-weight: 800;
    line-height: 1.02;
    letter-spacing: -0.02em;
    background: linear-gradient(135deg, #ffffff, #cfcbff);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    text-shadow: 0 6px 30px rgba(0, 0, 0, 0.5);
  }
  @keyframes rise {
    from { opacity: 0; transform: translateY(8px); }
    to { opacity: 1; transform: translateY(0); }
  }

  /* ========== Glass stats capsule ========== */
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
    width: 15px;
    height: 15px;
    color: rgba(236, 237, 245, 0.65);
  }
  .divider {
    width: 1px;
    height: 16px;
    background: rgba(255, 255, 255, 0.12);
  }

  .store-mark {
    display: inline-grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    overflow: hidden;
    color: #fff;
  }
  .store-mark .store-svg {
    width: 16px;
    height: 16px;
  }
  .store-mark.store-steam {
    background: #171d25; /* Steam dark blue */
    border: 1px solid rgba(255, 255, 255, 0.14);
  }
  .store-mark.store-epic {
    background: #2a2a2a; /* Epic dark grey */
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
    letter-spacing: 0.01em;
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
  .btn:disabled { opacity: 0.75; cursor: default; transform: none; }

  .btn-play {
    min-width: 220px;
    padding: 0 28px;
  }
  .btn-play.launching { animation: launchPulse 0.6s ease; }
  @keyframes launchPulse {
    0% { filter: brightness(1); }
    50% { filter: brightness(1.35); }
    100% { filter: brightness(1); }
  }

  .btn-icon {
    width: 52px;
    padding: 0;
  }

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
    top: 0;
    bottom: 0;
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
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .card .art-fallback {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    font-size: 50px;
    font-weight: 800;
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

  .card.selected {
    transform: translateY(-8px) scale(1.06);
  }
  .card.selected .art {
    box-shadow:
      0 0 0 2px rgba(255, 255, 255, 0.95),
      0 0 0 5px rgba(255, 255, 255, 0.08),
      0 22px 50px rgba(0, 0, 0, 0.55),
      0 0 30px rgba(255, 255, 255, 0.08);
  }
  .card.selected .card-name { color: #fff; }

  /* ========== Loading / empty ========== */
  .loading { display: grid; place-items: center; height: 100vh; }
  .spinner {
    width: 42px;
    height: 42px;
    border-radius: 50%;
    border: 3px solid rgba(255, 255, 255, 0.08);
    border-top-color: rgba(255, 255, 255, 0.8);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .empty {
    display: grid;
    place-items: center;
    height: 100vh;
    text-align: center;
    color: rgba(236, 237, 245, 0.7);
    padding: 24px;
  }
  .empty h2 { font-size: 22px; font-weight: 600; margin: 0 0 6px; }
  .empty p { margin: 0; max-width: 420px; color: rgba(236, 237, 245, 0.55); }
</style>
