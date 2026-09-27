import { expect, it } from "vitest";
import {
  acceptHealthState,
  needsHealthAttention,
  type HealthTaskState,
} from "./backupHealth";
import { normalizeAppSettings } from "./settings";
it("ignores stale revisions without losing a newly started task", () => {
  const current = { taskId: "a", revision: 8 } as HealthTaskState;
  expect(
    acceptHealthState(current, { taskId: "b", revision: 7 } as HealthTaskState),
  ).toBe(current);
  expect(
    acceptHealthState(current, { taskId: "b", revision: 9 } as HealthTaskState)
      .taskId,
  ).toBe("b");
});
it("does not hide failed sources inside an otherwise healthy archive", () => {
  expect(
    needsHealthAttention({
      code: "healthy",
      sources: [{ code: "missing" }],
      snapshots: [],
    } as never),
  ).toBe(true);
  expect(
    needsHealthAttention({
      code: "healthy",
      sources: [],
      snapshots: [],
    } as never),
  ).toBe(false);
});
it("normalizes health reminder thresholds", () => {
  expect(normalizeAppSettings().backupHealthStaleDays).toBe(7);
  expect(
    normalizeAppSettings({ backupHealthStaleDays: 0 }).backupHealthStaleDays,
  ).toBe(7);
  expect(
    normalizeAppSettings({ backupHealthStaleDays: 999 }).backupHealthStaleDays,
  ).toBe(365);
});
