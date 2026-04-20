<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { Game, GameSource } from "$lib/types";
  import { startGamepadLoop } from "$lib/gamepad";

  type Filter = "All" | GameSource;

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
      await openUrl(game.launchUrl);
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

  function formatPlaytime(mins: number | null): string {
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

  onMount(() => {
    (async () => {
      try {
        games = await invoke<Game[]>("list_games");
      } catch (e) {
        error = String(e);
      } finally {
        loading = false;
      }
    })();

    const stopPad = startGamepadLoop({
      onDir: (dir) => {
        if (dir === "left") move(-1);
        else if (dir === "right") move(1);
      },
      onConfirm: () => launch(),
      onSecondary: () => openSettings(),
    });

    return () => stopPad();
  });
</script>

<svelte:window on:keydown={onKey} />

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
        {#if current.heroUrl}
          <img
            class="hero-bg"
            src={current.heroUrl}
            alt=""
            onerror={(e) =>
              ((e.currentTarget as HTMLImageElement).style.display = "none")}
          />
        {:else if current.coverUrl}
          <img class="hero-bg hero-bg-cover" src={current.coverUrl} alt="" />
        {:else}
          <div class="hero-bg hero-bg-gradient"></div>
        {/if}
      {/key}

      <div class="scrim-left"></div>
      <div class="scrim-bottom"></div>

      <div class="hero-content">
        <div class="logo-area">
          {#key current.id}
            {#if current.logoUrl}
              <img
                class="wordmark"
                src={current.logoUrl}
                alt={current.name}
                onerror={(e) =>
                  ((e.currentTarget as HTMLImageElement).style.display = "none")}
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
              {current.source[0]}
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
              {#if game.coverUrl}
                <img
                  src={game.coverUrl}
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

  main {
    height: 100vh;
    display: grid;
    grid-template-rows: 1fr auto;
    gap: 0;
    background: #07080f;
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

  /* Logo area */
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

  /* Store mark: single letter in a minimal circle */
  .store-mark {
    display: inline-grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0;
  }
  .store-mark.store-steam {
    color: #8ec6ff;
    background: rgba(110, 180, 246, 0.14);
    border: 1px solid rgba(110, 180, 246, 0.35);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08);
  }
  .store-mark.store-epic {
    color: #f0f0f0;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.22);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08);
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
