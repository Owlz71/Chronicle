import { invoke } from "@tauri-apps/api/core";

export type SaveHitKind = "file" | "folder";

export type SaveSearchHit = {
  path: string;
  name: string;
  exact: boolean;
  /** RPG Maker XP/VX/VX Ace keep loose save files next to the game. */
  kind: SaveHitKind;
};

export type GameEngine =
  | "unity"
  | "rpgMaker2000"
  | "rpgMakerXp"
  | "rpgMakerVx"
  | "rpgMakerVxAce"
  | "rpgMakerMv"
  | "rpgMakerMz"
  | "unknown";

export type ExeCandidate = {
  path: string;
  name: string;
  likely: boolean;
};

export type DroppedGameInspection = {
  engine: GameEngine;
  root: string;
  exeCandidates: ExeCandidate[];
  recommendedExe: string | null;
};

/**
 * Searches the per-user AppData roots for folders named after a dropped
 * executable. Exact matches are returned first.
 */
export const searchAppdataSaves = (executablePath: string) =>
  invoke<SaveSearchHit[]>("search_appdata_saves", { executablePath });

/**
 * Recognises the engine of a dropped game folder or executable and lists the
 * executables that could be the game itself.
 */
export const inspectDroppedGame = (path: string) =>
  invoke<DroppedGameInspection>("inspect_dropped_game", { path });

/** Looks for save locations using what the engine detection learned. */
export const searchGameSaves = (
  root: string,
  executablePath: string,
  engine: GameEngine,
) => invoke<SaveSearchHit[]>("search_game_saves", { root, executablePath, engine });

export const isExecutablePath = (path: string) => /\.exe$/i.test(path.trim());

export function executableLabel(path: string): string {
  const segments = path.trim().split(/[\\/]/);
  return segments.at(-1) || path;
}

/** Engine names are proper nouns, so they are not translated. */
export function engineLabel(engine: GameEngine): string {
  const labels: Record<GameEngine, string> = {
    unity: "Unity",
    rpgMaker2000: "RPG Maker 2000 / 2003",
    rpgMakerXp: "RPG Maker XP",
    rpgMakerVx: "RPG Maker VX",
    rpgMakerVxAce: "RPG Maker VX Ace",
    rpgMakerMv: "RPG Maker MV",
    rpgMakerMz: "RPG Maker MZ",
    unknown: "",
  };
  return labels[engine];
}

