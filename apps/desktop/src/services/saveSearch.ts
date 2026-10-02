import { invoke } from "@tauri-apps/api/core";

export type SaveSearchHit = {
  path: string;
  name: string;
  exact: boolean;
};

/**
 * Searches the per-user AppData roots for folders named after a dropped
 * executable. Exact matches are returned first.
 */
export const searchAppdataSaves = (executablePath: string) =>
  invoke<SaveSearchHit[]>("search_appdata_saves", { executablePath });

export const isExecutablePath = (path: string) => /\.exe$/i.test(path.trim());

export function executableLabel(path: string): string {
  const segments = path.trim().split(/[\\/]/);
  return segments.at(-1) || path;
}
