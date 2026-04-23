import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdaterStatus =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "upToDate" }
  | { kind: "available"; version: string; notes?: string }
  | { kind: "downloading"; downloaded: number; total?: number }
  | { kind: "installing" }
  | { kind: "error"; message: string };

export const updater = $state<{ status: UpdaterStatus }>({
  status: { kind: "idle" },
});

let pending: Update | null = null;

export async function checkForUpdates(): Promise<boolean> {
  updater.status = { kind: "checking" };
  try {
    const update = await check();
    if (update) {
      pending = update;
      updater.status = {
        kind: "available",
        version: update.version,
        notes: update.body ?? undefined,
      };
      return true;
    }
    updater.status = { kind: "upToDate" };
    return false;
  } catch (e) {
    updater.status = { kind: "error", message: String(e) };
    return false;
  }
}

export async function downloadAndInstallPending(): Promise<void> {
  if (!pending) return;
  try {
    let total = 0;
    let downloaded = 0;
    await pending.downloadAndInstall((event) => {
      if (event.event === "Started") {
        total = event.data.contentLength ?? 0;
        downloaded = 0;
        updater.status = { kind: "downloading", downloaded, total };
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        updater.status = { kind: "downloading", downloaded, total };
      } else if (event.event === "Finished") {
        updater.status = { kind: "installing" };
      }
    });
    await relaunch();
  } catch (e) {
    updater.status = { kind: "error", message: String(e) };
  }
}

export function dismissUpdater() {
  updater.status = { kind: "idle" };
}
