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
  sizeBytes: number;
  playtimeMinutes: number | null;
}
