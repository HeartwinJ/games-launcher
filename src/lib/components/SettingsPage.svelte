<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { app } from "$lib/state.svelte";
  import {
    updater,
    checkForUpdates,
    downloadAndInstallPending,
  } from "$lib/updater.svelte";

  let rescanning = $state(false);

  const updateStatus = $derived(updater.status);

  const updateLabel = $derived.by(() => {
    switch (updateStatus.kind) {
      case "idle":
        return "Check for updates";
      case "checking":
        return "Checking…";
      case "upToDate":
        return "You're up to date";
      case "available":
        return `Install v${updateStatus.version}`;
      case "downloading": {
        if (!updateStatus.total) return "Downloading…";
        const pct = Math.floor((updateStatus.downloaded / updateStatus.total) * 100);
        return `Downloading ${pct}%`;
      }
      case "installing":
        return "Installing…";
      case "error":
        return "Retry check";
    }
  });

  const updateBusy = $derived(
    updateStatus.kind === "checking" ||
      updateStatus.kind === "downloading" ||
      updateStatus.kind === "installing",
  );

  async function onUpdateClick() {
    if (updateBusy) return;
    if (updateStatus.kind === "available") {
      await downloadAndInstallPending();
    } else {
      await checkForUpdates();
    }
  }

  async function toggleStore(store: "steam" | "epic") {
    const key = store === "steam" ? "showSteam" : "showEpic";
    const newValue = !app.prefs[key];
    app.prefs = { ...app.prefs, [key]: newValue };
    try {
      await invoke("set_pref", {
        key: store === "steam" ? "show_steam" : "show_epic",
        value: newValue ? "1" : "0",
      });
    } catch (e) {
      console.error("set_pref failed:", e);
    }
  }

  async function rescan() {
    if (rescanning) return;
    rescanning = true;
    try {
      // Re-enter the splash-scan pipeline. The existing scan:progress /
      // scan:done listeners in the root will reload the library.
      app.scanning = true;
      app.progress = { stage: "Starting up", done: 0, total: 0 };
      await invoke("init_scan");
    } catch (e) {
      console.error("rescan failed:", e);
      app.scanning = false;
    } finally {
      rescanning = false;
    }
  }

  async function openDataFolder() {
    try {
      await invoke("open_data_folder");
    } catch (e) {
      console.error("open_data_folder failed:", e);
    }
  }

  async function toggleFullscreen() {
    const newValue = !app.prefs.fullscreen;
    // Optimistic flip so the toggle reacts immediately.
    app.prefs = { ...app.prefs, fullscreen: newValue };
    try {
      // Applies the window state AND persists the pref in one round-trip.
      await invoke("set_fullscreen", { enabled: newValue });
    } catch (e) {
      // Revert on failure so the UI doesn't lie.
      app.prefs = { ...app.prefs, fullscreen: !newValue };
      console.error("set_fullscreen failed:", e);
    }
  }
</script>

<div class="page">
  <header>
    <h1>Settings</h1>
    <p class="muted">Configure library sources and app behavior.</p>
  </header>

  <section class="card">
    <h2>Stores</h2>
    <p class="muted">Choose which stores contribute to your library.</p>

    <div class="toggles">
      <button
        class="toggle"
        class:on={app.prefs.showSteam}
        onclick={() => toggleStore("steam")}
      >
        <span class="store-mark store-steam">
          <svg viewBox="0 0 24 24" class="store-svg" aria-hidden="true">
            <path fill="currentColor" d="M11.979 0C5.678 0 .511 4.86.022 11.037l6.432 2.658c.545-.371 1.203-.59 1.912-.59.063 0 .125.004.188.006l2.861-4.142V8.91c0-2.495 2.028-4.524 4.524-4.524 2.494 0 4.524 2.031 4.524 4.527s-2.03 4.525-4.524 4.525h-.105l-4.076 2.911c0 .052.004.105.004.159 0 1.875-1.515 3.396-3.39 3.396-1.635 0-3.016-1.173-3.331-2.727L.436 15.27C1.862 20.307 6.486 24 11.979 24c6.627 0 11.999-5.373 11.999-12S18.605 0 11.979 0zM7.54 18.21l-1.473-.61c.262.543.714.999 1.314 1.25 1.297.539 2.793-.076 3.332-1.375.263-.63.264-1.319.005-1.949s-.75-1.121-1.377-1.383c-.624-.26-1.29-.249-1.878-.03l1.523.63c.956.4 1.409 1.5 1.009 2.455-.397.957-1.497 1.41-2.454 1.012H7.54zm11.415-9.303c0-1.662-1.353-3.015-3.015-3.015-1.665 0-3.015 1.353-3.015 3.015 0 1.665 1.35 3.015 3.015 3.015 1.663 0 3.015-1.35 3.015-3.015zm-5.273-.005c0-1.252 1.013-2.266 2.265-2.266 1.249 0 2.266 1.014 2.266 2.266 0 1.251-1.017 2.265-2.266 2.265-1.253 0-2.265-1.014-2.265-2.265z" />
          </svg>
        </span>
        <div class="tg-text">
          <div class="tg-title">Steam</div>
          <div class="tg-sub">{app.prefs.showSteam ? "Visible" : "Hidden"}</div>
        </div>
        <div class="switch" class:switch-on={app.prefs.showSteam}>
          <div class="knob"></div>
        </div>
      </button>

      <button
        class="toggle"
        class:on={app.prefs.showEpic}
        onclick={() => toggleStore("epic")}
      >
        <span class="store-mark store-epic">
          <svg viewBox="0 0 24 24" class="store-svg" aria-hidden="true">
            <path fill="currentColor" d="M3.537 0C2.165 0 1.66.506 1.66 1.879V18.44a4.262 4.262 0 00.02.433c.031.3.037.59.316.92.027.033.311.245.311.245.153.075.258.13.43.2l8.335 3.491c.433.199.614.276.928.27h.002c.314.006.495-.071.928-.27l8.335-3.492c.172-.07.277-.124.43-.2 0 0 .284-.211.311-.243.28-.33.285-.621.316-.92a4.261 4.261 0 00.02-.434V1.879c0-1.373-.506-1.88-1.878-1.88zm13.366 3.11h.68c1.138 0 1.688.553 1.688 1.696v1.88h-1.374v-1.8c0-.369-.17-.54-.523-.54h-.235c-.367 0-.537.17-.537.539v5.81c0 .369.17.54.537.54h.262c.353 0 .523-.171.523-.54V8.619h1.373v2.143c0 1.144-.562 1.71-1.7 1.71h-.694c-1.138 0-1.7-.566-1.7-1.71V4.82c0-1.144.562-1.709 1.7-1.709zm-12.186.08h3.114v1.274H6.117v2.603h1.648v1.275H6.117v2.774h1.74v1.275h-3.14zm3.816 0h2.198c1.138 0 1.7.564 1.7 1.708v2.445c0 1.144-.562 1.71-1.7 1.71h-.799v3.338h-1.4zm4.53 0h1.4v9.201h-1.4zm-3.13 1.235v3.392h.575c.354 0 .523-.171.523-.54V4.965c0-.368-.17-.54-.523-.54zm-1.145 5.19h8.014l-4.09 1.348z" />
          </svg>
        </span>
        <div class="tg-text">
          <div class="tg-title">Epic Games</div>
          <div class="tg-sub">{app.prefs.showEpic ? "Visible" : "Hidden"}</div>
        </div>
        <div class="switch" class:switch-on={app.prefs.showEpic}>
          <div class="knob"></div>
        </div>
      </button>
    </div>
  </section>

  <section class="card">
    <h2>Window</h2>
    <p class="muted">Toggle applies immediately and is remembered for next launch.</p>

    <div class="toggles">
      <button
        class="toggle"
        class:on={app.prefs.fullscreen}
        onclick={toggleFullscreen}
      >
        <span class="row-icon">
          <svg viewBox="0 0 24 24" class="glyph" aria-hidden="true"
            fill="none" stroke="currentColor" stroke-width="1.7"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M8 3H5a2 2 0 0 0-2 2v3" />
            <path d="M21 8V5a2 2 0 0 0-2-2h-3" />
            <path d="M3 16v3a2 2 0 0 0 2 2h3" />
            <path d="M16 21h3a2 2 0 0 0 2-2v-3" />
          </svg>
        </span>
        <div class="tg-text">
          <div class="tg-title">Fullscreen</div>
          <div class="tg-sub">
            {app.prefs.fullscreen ? "On" : "Off"}
          </div>
        </div>
        <div class="switch" class:switch-on={app.prefs.fullscreen}>
          <div class="knob"></div>
        </div>
      </button>
    </div>
  </section>

  <section class="card">
    <h2>Library</h2>
    <p class="muted">Rebuild the library or inspect the on-disk cache.</p>

    <div class="actions">
      <button class="action-btn" onclick={rescan} disabled={rescanning}>
        <svg viewBox="0 0 24 24" class="icon" aria-hidden="true"
          fill="none" stroke="currentColor" stroke-width="1.7"
          stroke-linecap="round" stroke-linejoin="round">
          <polyline points="23 4 23 10 17 10" />
          <polyline points="1 20 1 14 7 14" />
          <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
        </svg>
        <span>{rescanning ? "Scanning…" : "Rescan library"}</span>
      </button>

      <button class="action-btn" onclick={openDataFolder}>
        <svg viewBox="0 0 24 24" class="icon" aria-hidden="true"
          fill="none" stroke="currentColor" stroke-width="1.7"
          stroke-linejoin="round">
          <path d="M3 6a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6Z" />
        </svg>
        <span>Open data folder</span>
      </button>
    </div>
  </section>

  <section class="card">
    <h2>Updates</h2>
    <p class="muted">Automatic check on launch. You can also check now.</p>

    <div class="actions">
      <button
        class="action-btn"
        onclick={onUpdateClick}
        disabled={updateBusy || updateStatus.kind === "upToDate"}
      >
        <svg viewBox="0 0 24 24" class="icon" aria-hidden="true"
          fill="none" stroke="currentColor" stroke-width="1.7"
          stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 3v12" />
          <path d="M7 10l5 5 5-5" />
          <path d="M5 21h14" />
        </svg>
        <span>{updateLabel}</span>
      </button>
    </div>

    {#if updateStatus.kind === "available" && updateStatus.notes}
      <p class="muted notes">{updateStatus.notes}</p>
    {:else if updateStatus.kind === "error"}
      <p class="muted notes error">{updateStatus.message}</p>
    {/if}
  </section>

  <section class="card about">
    <h2>About</h2>
    <div class="about-body">
      <div class="brand">
        <span class="brand-dot"></span>
        <span class="brand-title">Games Launcher</span>
        <span class="brand-version">v0.1.0</span>
      </div>
      <p class="muted">
        A minimal, controller-friendly launcher that aggregates installed Steam
        and Epic games into a single, TV-style grid. Scans your library on
        startup, caches artwork locally, and tracks playtime natively via a
        process-based watcher.
      </p>
      <p class="muted tech">
        Built with Tauri 2 · SvelteKit · Rust · SQLite.
      </p>
    </div>
  </section>
</div>

<style>
  .page {
    max-width: 780px;
    margin: 0 auto;
    padding: 48px 32px 80px;
    display: grid;
    gap: 24px;
    animation: rise 0.28s ease both;
    /* Hide the scrollbar while keeping scroll functional. */
    scrollbar-width: none;
  }
  .page::-webkit-scrollbar {
    display: none;
  }
  @keyframes rise {
    from { opacity: 0; transform: translateY(8px); }
    to { opacity: 1; transform: translateY(0); }
  }

  header {
    margin-bottom: 4px;
  }
  h1 {
    font-size: 32px;
    font-weight: 700;
    letter-spacing: -0.015em;
    margin: 0 0 6px;
    background: linear-gradient(135deg, #fff, #c9c4ff);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  .muted {
    color: rgba(236, 237, 245, 0.55);
    font-size: 13.5px;
    margin: 0;
  }

  .card {
    padding: 22px 24px;
    border-radius: 18px;
    background: rgba(255, 255, 255, 0.04);
    backdrop-filter: blur(22px) saturate(160%);
    -webkit-backdrop-filter: blur(22px) saturate(160%);
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.08),
      0 12px 30px rgba(0, 0, 0, 0.25);
  }
  h2 {
    font-size: 14px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: rgba(236, 237, 245, 0.7);
    margin: 0 0 6px;
  }
  .card p.muted {
    margin-bottom: 16px;
  }

  .toggles {
    display: grid;
    gap: 10px;
  }
  .toggle {
    all: unset;
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    cursor: pointer;
    transition: background 0.18s ease, border-color 0.18s ease;
  }
  .toggle:hover,
  .toggle:focus {
    background: rgba(255, 255, 255, 0.07);
    border-color: rgba(255, 255, 255, 0.14);
    outline: none;
  }

  .store-mark {
    display: inline-grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    color: #fff;
  }
  .store-mark .store-svg {
    width: 18px;
    height: 18px;
  }
  .store-mark.store-steam {
    background: #171d25;
    border: 1px solid rgba(255, 255, 255, 0.14);
  }
  .store-mark.store-epic {
    background: #2a2a2a;
    border: 1px solid rgba(255, 255, 255, 0.18);
  }

  .row-icon {
    display: inline-grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 10px;
    color: rgba(236, 237, 245, 0.8);
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
  }
  .row-icon .glyph {
    width: 18px;
    height: 18px;
  }

  .tg-text { display: grid; gap: 2px; }
  .tg-title { font-size: 14px; font-weight: 600; color: #fff; }
  .tg-sub { font-size: 11.5px; color: rgba(236, 237, 245, 0.55); }

  .switch {
    width: 40px;
    height: 22px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.1);
    position: relative;
    transition: background 0.2s ease;
  }
  .switch .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.35);
    transition: left 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .switch.switch-on {
    background: linear-gradient(135deg, #8b7bff, #1dd3da);
  }
  .switch.switch-on .knob { left: 21px; }

  .actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .action-btn {
    all: unset;
    display: inline-flex;
    align-items: center;
    gap: 10px;
    padding: 10px 18px;
    border-radius: 12px;
    font-size: 13.5px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.92);
    cursor: pointer;
    background: linear-gradient(
      180deg,
      rgba(255, 255, 255, 0.1),
      rgba(255, 255, 255, 0.04)
    );
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.1);
    transition: background 0.15s ease, transform 0.15s ease;
  }
  .action-btn:hover { background: rgba(255, 255, 255, 0.14); transform: translateY(-1px); }
  .action-btn:focus {
    outline: none;
    border-color: rgba(255, 255, 255, 0.4);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.1),
      0 0 0 2px rgba(255, 255, 255, 0.14);
  }
  .action-btn:disabled { opacity: 0.6; cursor: default; transform: none; }
  .action-btn .icon { width: 16px; height: 16px; color: rgba(236, 237, 245, 0.75); }

  .about-body { display: grid; gap: 10px; }
  .brand {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .brand-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: linear-gradient(135deg, #8b7bff, #1dd3da);
    box-shadow: 0 0 14px rgba(139, 123, 255, 0.6);
    transform: translateY(1px);
  }
  .brand-title {
    font-size: 18px;
    font-weight: 700;
    color: #fff;
  }
  .brand-version {
    font-size: 12px;
    color: rgba(236, 237, 245, 0.5);
    font-variant-numeric: tabular-nums;
  }
  .tech { font-size: 12px; }

  .notes {
    margin-top: 12px;
    font-size: 12.5px;
    line-height: 1.5;
    white-space: pre-wrap;
  }
  .notes.error {
    color: rgba(255, 170, 170, 0.75);
  }
</style>
