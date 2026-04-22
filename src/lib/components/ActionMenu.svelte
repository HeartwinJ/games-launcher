<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { openPath } from "@tauri-apps/plugin-opener";
  import type { Game } from "$lib/types";
  import { app } from "$lib/state.svelte";

  interface Props {
    game: Game;
    onClose: () => void;
  }
  let { game, onClose }: Props = $props();

  let modalEl: HTMLDivElement | undefined = $state();
  let status = $state<string | null>(null);

  async function run(fn: () => Promise<void>) {
    try {
      await fn();
      onClose();
    } catch (e) {
      status = String(e);
    }
  }

  const openFolder = () =>
    run(async () => {
      await openPath(game.installPath);
    });
  const copyPath = () =>
    run(async () => {
      await navigator.clipboard.writeText(game.installPath);
    });
  const hide = () =>
    run(async () => {
      await invoke("hide_game", { id: game.id });
      app.games = app.games.filter((g) => g.id !== game.id);
    });
  const resetPlaytime = () =>
    run(async () => {
      await invoke("reset_playtime", { id: game.id });
    });
  const uninstall = () =>
    run(async () => {
      await invoke("uninstall_game", { id: game.id });
    });

  $effect(() => {
    // Auto-focus the first action when the modal mounts.
    queueMicrotask(() => {
      const first = modalEl?.querySelector<HTMLButtonElement>(
        "button:not([data-no-nav])",
      );
      first?.focus({ preventScroll: true });
    });
  });
</script>

<!-- Backdrop: dim + blur + click-to-close. Excluded from spatial nav + Tab. -->
<button
  class="backdrop"
  onclick={onClose}
  aria-label="Close menu"
  tabindex="-1"
  data-no-nav
></button>

<div
  class="modal"
  role="dialog"
  aria-modal="true"
  aria-labelledby="action-menu-title"
  data-nav-zone="menu"
  tabindex="-1"
  bind:this={modalEl}
>
  <header>
    <div class="title" id="action-menu-title">{game.name}</div>
    <div class="subtitle">Manage game</div>
  </header>

  <div class="items">
    <button class="item" onclick={openFolder}>
      <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"
        fill="none" stroke="currentColor" stroke-width="1.7"
        stroke-linejoin="round">
        <path d="M3 6a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6Z" />
      </svg>
      <span>Open install folder</span>
    </button>

    <button class="item" onclick={copyPath}>
      <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"
        fill="none" stroke="currentColor" stroke-width="1.7"
        stroke-linecap="round" stroke-linejoin="round">
        <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
        <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
      </svg>
      <span>Copy install path</span>
    </button>

    <button class="item" onclick={hide}>
      <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"
        fill="none" stroke="currentColor" stroke-width="1.7"
        stroke-linecap="round" stroke-linejoin="round">
        <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" />
        <line x1="1" y1="1" x2="23" y2="23" />
      </svg>
      <span>Hide from library</span>
    </button>

    <button class="item" onclick={resetPlaytime}>
      <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"
        fill="none" stroke="currentColor" stroke-width="1.7"
        stroke-linecap="round" stroke-linejoin="round">
        <polyline points="1 4 1 10 7 10" />
        <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10" />
      </svg>
      <span>Reset playtime</span>
    </button>

    <div class="divider"></div>

    <button class="item danger" onclick={uninstall}>
      <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"
        fill="none" stroke="currentColor" stroke-width="1.7"
        stroke-linecap="round" stroke-linejoin="round">
        <polyline points="3 6 5 6 21 6" />
        <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
        <path d="M10 11v6M14 11v6" />
        <path d="M9 6V4a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v2" />
      </svg>
      <span>Uninstall</span>
    </button>

    {#if status}
      <div class="status">{status}</div>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    all: unset;
    position: fixed;
    inset: 0;
    z-index: 90;
    background: rgba(7, 8, 15, 0.55);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    cursor: default;
    animation: backdropIn 0.22s ease both;
  }
  @keyframes backdropIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .modal {
    position: fixed;
    top: 50%;
    left: 50%;
    /* Base centering transform — animation keyframes extend this briefly. */
    transform: translate(-50%, -50%);
    z-index: 91;
    width: min(440px, calc(100vw - 40px));
    max-height: calc(100vh - 80px);
    display: flex;
    flex-direction: column;
    border-radius: 18px;
    background: rgba(18, 20, 32, 0.9);
    backdrop-filter: blur(26px) saturate(160%);
    -webkit-backdrop-filter: blur(26px) saturate(160%);
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.08),
      0 40px 80px rgba(0, 0, 0, 0.6);
    outline: none;
    overflow: hidden;
    animation: modalIn 0.22s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  @keyframes modalIn {
    from {
      opacity: 0;
      transform: translate(-50%, -46%) scale(0.96);
    }
    to {
      opacity: 1;
      transform: translate(-50%, -50%) scale(1);
    }
  }

  header {
    padding: 20px 22px 14px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }
  .title {
    font-size: 16px;
    font-weight: 700;
    color: #fff;
    letter-spacing: -0.005em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .subtitle {
    font-size: 10.5px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: rgba(236, 237, 245, 0.45);
    margin-top: 4px;
  }

  .items {
    padding: 8px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .item {
    all: unset;
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 10px 12px;
    border-radius: 9px;
    font-size: 13.5px;
    color: rgba(236, 237, 245, 0.9);
    cursor: pointer;
    box-sizing: border-box;
    transition: background 0.12s ease, color 0.12s ease;
  }
  .item .icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    color: rgba(236, 237, 245, 0.65);
  }
  .item:hover,
  .item:focus {
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
    outline: none;
  }
  .item:focus {
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.18);
  }
  .item:focus .icon,
  .item:hover .icon { color: #fff; }

  .item.danger { color: #ff9a9a; }
  .item.danger .icon { color: #ff9a9a; }
  .item.danger:hover,
  .item.danger:focus {
    background: rgba(255, 85, 85, 0.14);
    color: #ffb4b4;
  }

  .divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.08);
    margin: 4px 6px;
  }

  .status {
    margin: 8px 6px 4px;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgba(255, 85, 85, 0.1);
    border: 1px solid rgba(255, 85, 85, 0.25);
    color: #ffb4b4;
    font-size: 11.5px;
  }
</style>
