import { locale, t } from "./i18n";

/** Formats a byte count for display, using an em dash when nothing was recorded. */
export function formatBytes(bytes: number): string {
  if (!bytes) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const unit = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** unit).toFixed(unit > 1 ? 1 : 0)} ${units[unit]}`;
}

/** Formats a timestamp as "today HH:MM" or, for other days, a short date and time. */
export function formatTime(timestamp?: number): string {
  if (!timestamp) return t("尚未备份");
  const date = new Date(timestamp);
  const today = new Date();
  if (date.toDateString() === today.toDateString()) {
    return t("今天 {time}", { time: date.toLocaleTimeString(locale.value, { hour: "2-digit", minute: "2-digit", hour12: false }) });
  }
  return date.toLocaleString(locale.value, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", hour12: false });
}
