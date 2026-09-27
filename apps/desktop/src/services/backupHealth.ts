import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { t } from "./i18n";
export type HealthCode =
  | "healthy"
  | "missing"
  | "permission_denied"
  | "unavailable"
  | "unbound"
  | "hash_mismatch"
  | "read_error"
  | "skipped"
  | "changed_during_check"
  | "no_snapshots"
  | "pending_changes"
  | "stale_changes"
  | "cancelled";
export interface HealthItem {
  id: string;
  path: string;
  code: HealthCode;
  detail: string;
}
export interface HealthEntryResult {
  entryId: string;
  name: string;
  sources: HealthItem[];
  snapshots: HealthItem[];
  comparison: "equal" | "different" | "unknown";
  code: HealthCode;
  lastSnapshotAt: number | null;
}
export interface HealthReport {
  formatVersion: number;
  startedAt: number;
  completedAt: number;
  entries: HealthEntryResult[];
}
export interface HealthTaskState {
  taskId: string;
  revision: number;
  status: string;
  checked: number;
  total: number;
  currentEntryId: string | null;
  entries: HealthEntryResult[];
  error: string | null;
}
export const emptyHealthState = (): HealthTaskState => ({
  taskId: "",
  revision: 0,
  status: "idle",
  checked: 0,
  total: 0,
  currentEntryId: null,
  entries: [],
  error: null,
});
export const acceptHealthState = (
  current: HealthTaskState,
  incoming: HealthTaskState,
) => (incoming.revision > current.revision ? incoming : current);
export const needsHealthAttention = (entry: HealthEntryResult) =>
  entry.code !== "healthy" ||
  [...entry.sources, ...entry.snapshots].some((i) => i.code !== "healthy");
export const healthSupported = () =>
  isTauri() && navigator.userAgent.includes("Windows");
export const startHealthCheck = () =>
  invoke<string>("start_backup_health_check");
export const cancelHealthCheck = (taskId: string) =>
  invoke<void>("cancel_backup_health_check", { taskId });
export const getHealthState = () =>
  invoke<HealthTaskState>("get_backup_health_state");
export const loadHealthReport = () =>
  invoke<HealthReport | null>("load_backup_health_report");
export const subscribeHealthProgress = (
  callback: (s: HealthTaskState) => void,
) =>
  listen<HealthTaskState>("backup-health-progress", (event) =>
    callback(event.payload),
  );
export function healthLabel(code: string): string {
  const labels: Record<string, string> = {
    healthy: "检查正常",
    missing: "路径不存在",
    permission_denied: "权限不足",
    unavailable: "位置不可访问",
    unbound: "未绑定本机路径",
    hash_mismatch: "校验不一致",
    read_error: "读取失败",
    skipped: "已跳过，未检查",
    changed_during_check: "检查期间发生变化，请重检",
    no_snapshots: "尚未创建备份",
    pending_changes: "存在未备份变化",
    stale_changes: "未备份变化已达到提醒天数",
    cancelled: "已取消",
    idle: "尚未检查",
    running: "检查中",
    cancelling: "正在取消",
    completed: "检查完成",
    failed: "检查失败",
    health_cache_corrupt: "检查缓存损坏，请重新检查",
  };
  return t(labels[code] ?? code);
}
