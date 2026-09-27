import { describe, expect, it } from "vitest";
import {
  defaultBackupTrigger,
  validateBackupTrigger,
  acceptBackupRuntime,
} from "./backupAutomation";
describe("backup automation", () => {
  it("defaults to the legacy file-change mode and requires an executable for exit mode", () => {
    expect(defaultBackupTrigger().mode).toBe("file_change");
    expect(
      validateBackupTrigger({ ...defaultBackupTrigger(), mode: "game_exit" }),
    ).toBe("backup_executable_required");
    expect(
      validateBackupTrigger({
        ...defaultBackupTrigger(),
        mode: "game_exit",
        executablePath: "C:\\game\\game.exe",
      }),
    ).toBeNull();
    expect(
      validateBackupTrigger({ ...defaultBackupTrigger(), quietSeconds: 301 }),
    ).toBe("backup_invalid_quiet_seconds");
  });
  it("ignores runtime events from an earlier configuration", () => {
    const current = {
      entryId: "a",
      generation: 2,
      status: "running",
      reasonCode: null,
    };
    expect(
      acceptBackupRuntime(current, {
        ...current,
        generation: 1,
        status: "waiting",
      }),
    ).toBe(current);
  });
});
