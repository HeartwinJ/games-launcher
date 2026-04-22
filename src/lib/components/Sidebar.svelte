<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { app } from "$lib/state.svelte";

  interface Props {
    inert?: boolean;
  }
  let { inert = false }: Props = $props();

  async function exit() {
    try {
      await getCurrentWindow().close();
    } catch (e) {
      console.error("exit failed:", e);
    }
  }
</script>

<aside class="sidebar" {inert} data-nav-zone="sidebar">
  <nav class="sidebar-main">
    <button
      class="item"
      class:active={app.page === "home"}
      onclick={() => (app.page = "home")}
      aria-label="Home"
    >
      <span class="item-icon">
        <svg viewBox="0 0 24 24" aria-hidden="true"
          fill="none" stroke="currentColor" stroke-width="1.8"
          stroke-linecap="round" stroke-linejoin="round">
          <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2h-4v-7h-6v7H5a2 2 0 0 1-2-2z" />
        </svg>
      </span>
      <span class="item-label">Home</span>
    </button>

    <button
      class="item"
      class:active={app.page === "settings"}
      onclick={() => (app.page = "settings")}
      aria-label="Settings"
    >
      <span class="item-icon">
        <svg viewBox="0 0 24 24" aria-hidden="true"
          fill="none" stroke="currentColor" stroke-width="1.8"
          stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
        </svg>
      </span>
      <span class="item-label">Settings</span>
    </button>
  </nav>

  <div class="sidebar-bottom">
    <button class="item" onclick={exit} aria-label="Exit">
      <span class="item-icon">
        <svg viewBox="0 0 24 24" aria-hidden="true"
          fill="none" stroke="currentColor" stroke-width="1.8"
          stroke-linecap="round" stroke-linejoin="round">
          <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
          <polyline points="16 17 21 12 16 7" />
          <line x1="21" y1="12" x2="9" y2="12" />
        </svg>
      </span>
      <span class="item-label">Exit</span>
    </button>
  </div>
</aside>

<style>
  .sidebar {
    position: fixed;
    left: 0;
    top: 0;
    bottom: 0;
    width: 64px;
    z-index: 50;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 14px 8px;
    background: linear-gradient(
      180deg,
      rgba(7, 8, 15, 0.55) 0%,
      rgba(7, 8, 15, 0.78) 100%
    );
    backdrop-filter: blur(22px) saturate(160%);
    -webkit-backdrop-filter: blur(22px) saturate(160%);
    border-right: 1px solid rgba(255, 255, 255, 0.06);
    transition: width 0.28s cubic-bezier(0.2, 0.8, 0.2, 1);
    overflow: hidden;
  }
  .sidebar:hover,
  .sidebar:focus-within {
    width: 220px;
    box-shadow: 24px 0 60px rgba(0, 0, 0, 0.35);
  }

  .sidebar-main {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex: 1;
    justify-content: center;
  }

  /* Exit is hidden (and un-focusable) while the sidebar is collapsed. */
  .sidebar-bottom {
    opacity: 0;
    visibility: hidden;
    transform: translateY(6px);
    transition:
      opacity 0.2s ease,
      transform 0.24s ease,
      visibility 0s linear 0.2s;
  }
  .sidebar:hover .sidebar-bottom,
  .sidebar:focus-within .sidebar-bottom {
    opacity: 1;
    visibility: visible;
    transform: translateY(0);
    transition-delay: 0s, 0s, 0s;
  }

  .item {
    all: unset;
    display: flex;
    align-items: center;
    height: 44px;
    width: 100%;
    border-radius: 12px;
    color: rgba(236, 237, 245, 0.72);
    cursor: pointer;
    overflow: hidden;
    white-space: nowrap;
    transition:
      background 0.18s ease,
      color 0.18s ease,
      box-shadow 0.18s ease;
  }
  /* Icon cell is the full inner width of the collapsed sidebar so the
     icon sits perfectly centered when the label is hidden. */
  .item-icon {
    width: 48px;
    height: 44px;
    display: grid;
    place-items: center;
    flex-shrink: 0;
  }
  .item-icon svg {
    width: 22px;
    height: 22px;
  }
  .item-label {
    font-size: 13.5px;
    font-weight: 500;
    letter-spacing: 0.01em;
    max-width: 0;
    opacity: 0;
    padding-right: 12px;
    transition:
      max-width 0.28s cubic-bezier(0.2, 0.8, 0.2, 1),
      opacity 0.22s ease;
  }
  .sidebar:hover .item-label,
  .sidebar:focus-within .item-label {
    max-width: 160px;
    opacity: 1;
  }

  .item:hover {
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
  }
  .item:focus {
    outline: none;
    color: #fff;
    background: linear-gradient(
      180deg,
      rgba(255, 255, 255, 0.12),
      rgba(255, 255, 255, 0.05)
    );
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.12),
      0 0 0 1px rgba(255, 255, 255, 0.2);
  }
  .item.active {
    color: #fff;
    background: rgba(255, 255, 255, 0.08);
  }
</style>
