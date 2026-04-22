export type GameSource = "Steam" | "Epic";

export interface Game {
  id: string;
  name: string;
  source: GameSource;
  appId: string;
  installPath: string;
  launchUrl: string;
  coverUrl: string | null;
  heroUrl: string | null;
  logoUrl: string | null;
  coverLocal: string | null;
  heroLocal: string | null;
  logoLocal: string | null;
  sizeBytes: number;
  playtimeMinutes: number;
}

export interface ScanProgress {
  stage: string;
  done: number;
  total: number;
}

export interface PlaytimeUpdate {
  gameId: string;
  playtimeMinutes: number;
}

export interface Prefs {
  showSteam: boolean;
  showEpic: boolean;
}

export type PageName = "home" | "settings";
