<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref } from "vue";
import { X } from "@lucide/vue";
import { t, locale } from "../services/i18n";
import {
  acceptHealthState,
  cancelHealthCheck,
  emptyHealthState,
  getHealthState,
  healthLabel,
  healthSupported,
  loadHealthReport,
  needsHealthAttention,
  startHealthCheck,
  subscribeHealthProgress,
  type HealthReport,
} from "../services/backupHealth";
const emit = defineEmits<{
  close: [];
  "open-archive": [id: string];
  "edit-sources": [id: string];
  "backup-now": [id: string];
}>();
const state = ref(emptyHealthState());
const report = ref<HealthReport | null>(null);
const error = ref("");
const busy = ref(false);
const onlyProblems = ref(false);
const closeButton = ref<HTMLButtonElement>();
let dispose: (() => void) | undefined;
let disposed = false;
const running = computed(() =>
  ["running", "cancelling"].includes(state.value.status),
);
const entries = computed(() =>
  (state.value.status === "idle"
    ? (report.value?.entries ?? [])
    : state.value.entries
  ).filter((e) => !onlyProblems.value || needsHealthAttention(e)),
);
const previousFocus = document.activeElement as HTMLElement | null;
async function start() {
  busy.value = true;
  error.value = "";
  try {
    await startHealthCheck();
    state.value = acceptHealthState(state.value, await getHealthState());
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}
async function cancel() {
  try {
    await cancelHealthCheck(state.value.taskId);
  } catch (e) {
    error.value = String(e);
  }
}
async function copy() {
  try {
    await navigator.clipboard.writeText(
      JSON.stringify(
        state.value.status === "idle" ? report.value : state.value,
        null,
        2,
      ),
    );
  } catch (e) {
    error.value = String(e);
  }
}
function keyboard(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.stopPropagation();
    emit("close");
  }
  if (event.key === "Tab") {
    const root = closeButton.value?.closest("section");
    const nodes = Array.from(
      root?.querySelectorAll<HTMLElement>(
        'button:not(:disabled),input:not(:disabled),summary,[tabindex="0"]',
      ) ?? [],
    );
    const first = nodes[0],
      last = nodes.at(-1);
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last?.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first?.focus();
    }
  }
}
onMounted(async () => {
  closeButton.value?.focus();
  if (!healthSupported()) return;
  try {
    const unlisten = await subscribeHealthProgress((s) => {
      state.value = acceptHealthState(state.value, s);
    });
    if (disposed) {
      unlisten();
      return;
    }
    dispose = unlisten;
    state.value = acceptHealthState(state.value, await getHealthState());
    report.value = await loadHealthReport();
  } catch (e) {
    error.value = String(e);
  }
});
onBeforeUnmount(() => {
  disposed = true;
  dispose?.();
  previousFocus?.focus();
});
</script>
<template>
  <div
    class="backup-health-backdrop"
    @click.self="emit('close')"
    @keydown="keyboard"
  >
    <section
      role="dialog"
      aria-modal="true"
      aria-labelledby="backup-health-title"
    >
      <header>
        <h2 id="backup-health-title">{{ t("备份健康检查") }}</h2>
        <button
          ref="closeButton"
          :aria-label="t('关闭')"
          @click="emit('close')"
        >
          <X :size="20" />
        </button>
      </header>
      <p class="health-hint">
        {{ t("只检查本地快照和来源，不修改文件。哈希通过不代表已验证恢复。") }}
      </p>
      <p v-if="!healthSupported()">{{ t("仅 Windows 桌面版可用") }}</p>
      <div class="health-toolbar">
        <button
          class="health-start"
          :disabled="!healthSupported() || running || busy"
          @click="start"
        >
          {{ t("开始检查") }}</button
        ><button
          v-if="running"
          :disabled="state.status === 'cancelling'"
          @click="cancel"
        >
          {{ t("取消检查") }}</button
        ><label
          ><input v-model="onlyProblems" type="checkbox" />{{
            t("只看需要处理")
          }}</label
        ><button @click="copy">{{ t("复制诊断信息") }}</button>
      </div>
      <p class="health-status" aria-live="polite">
        {{ state.status === 'idle' && report ? '' : healthLabel(state.status) }}
        <span v-if="state.taskId"
          >· {{ state.checked }} / {{ state.total }}</span
        ><span v-if="state.status === 'idle' && report">
          {{ t("上次检查") }}
          {{ new Date(report.completedAt).toLocaleString(locale) }}</span
        >
      </p>
      <progress v-if="running" :value="state.checked" :max="state.total || 1" />
      <p v-if="error || state.error" role="alert">
        {{ healthLabel(error || state.error || "") }}
      </p>
      <div class="health-results">
        <article v-for="entry in entries" :key="entry.entryId">
          <h3>
            {{ entry.name }} <small class="health-badge" :class="{ 'needs-attention': needsHealthAttention(entry) }">{{ healthLabel(entry.code) }}</small>
          </h3>
          <p v-if="entry.lastSnapshotAt">
            {{ t("最后备份") }} ·
            {{ new Date(entry.lastSnapshotAt).toLocaleString(locale) }}
          </p>
          <p v-if="entry.comparison === 'unknown'">
            {{ t("当前内容变化未完整检查（注册表不参与变化比较）") }}
          </p>
          <details>
            <summary>
              {{ t("来源和快照详情") }} ·
              {{ entry.sources.length + entry.snapshots.length }}
            </summary>
            <div
              v-for="item in [...entry.sources, ...entry.snapshots]"
              :key="item.id"
              class="health-item"
            >
              <b>{{ healthLabel(item.code) }}</b
              ><span>{{ item.path || item.id }}</span
              ><small v-if="item.detail">{{ item.detail }}</small>
            </div>
          </details>
          <div class="health-actions">
            <button @click="emit('open-archive', entry.entryId)">
              {{ t("打开存档") }}</button
            ><button @click="emit('edit-sources', entry.entryId)">
              {{ t("编辑来源") }}</button
            ><button
              :disabled="running"
              @click="emit('backup-now', entry.entryId)"
            >
              {{ t("立即备份") }}
            </button>
          </div>
        </article>
        <p v-if="!entries.length && !running">
          {{ t("没有可显示的检查结果") }}
        </p>
      </div>
      <footer>
        <span>{{
          t("关闭窗口后检查仍会继续。异常快照可核对云端副本，不会自动修复。")
        }}</span
        ><button @click="emit('close')">{{ t("关闭") }}</button>
      </footer>
    </section>
  </div>
</template>
<style scoped>
.backup-health-backdrop {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: grid;
  place-items: center;
  padding: 24px;
  background: #18181b99;
  backdrop-filter: blur(3px);
}
section {
  width: min(820px, 100%);
  max-height: calc(100vh - 48px);
  display: flex;
  flex-direction: column;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border-2);
  border-radius: 14px;
  padding: 20px;
  font-size: 12px;
  line-height: 1.5;
  box-shadow: 0 24px 80px var(--shadow-color);
  gap: 12px;
}
header,
.health-toolbar,
.health-actions,
footer {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
header {
  justify-content: space-between;
}
h2,
h3,
p {
  margin: 0;
}
h2 {
  font-size: 19px;
}
h3 {
  font-size: 13px;
  overflow-wrap: anywhere;
}
small,
.health-hint,
footer {
  color: var(--text-2);
  font-size: 11px;
}
button {
  border: 1px solid var(--border-2);
  border-radius: 8px;
  min-height: 32px;
  padding: 6px 10px;
  font: inherit;
  font-size: 11px;
  color: var(--primary-dark);
  background: var(--surface);
  cursor: pointer;
}
button:disabled {
  opacity: 0.5;
  cursor: default;
}
button:focus-visible,
input:focus-visible,
summary:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 3px;
}
.health-results {
  overflow: auto;
  min-height: 0;
  border: 1px solid var(--border);
  border-radius: 9px;
}
article {
  padding: 14px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
}
article + article { border-top: 1px solid var(--border); }
article > p { grid-column: 1; color: var(--text-2); font-size: 11px; }
article > details { grid-column: 1 / -1; }
article h3 { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; }
h3 small {
  font-size: 10px;
}
.health-item {
  display: grid;
  gap: 4px;
  padding: 10px 0;
  border-bottom: 1px solid var(--border);
  overflow-wrap: anywhere;
  font-size: 11px;
}
progress {
  width: 100%;
  accent-color: var(--primary);
}
footer span {
  flex: 1;
}
footer { border-top: 1px solid var(--border); padding-top: 12px; }
header button { display: grid; place-items: center; width: 34px; padding: 0; border: 0; background: transparent; }
.health-toolbar { padding: 4px 0; }
.health-toolbar label { margin-left: auto; font-size: 11px; }
.health-toolbar input { accent-color: var(--primary); }
.health-start { color: var(--on-primary); background: var(--primary); border-color: var(--primary); }
button:not(:disabled):hover { background: var(--hover); }
.health-start:not(:disabled):hover { background: var(--primary-dark); }
.health-status { color: var(--text-2); font-size: 11px; }
.health-badge { padding: 2px 7px; border-radius: 5px; color: var(--success); background: var(--success-soft); font-weight: 600; }
.health-badge.needs-attention { color: var(--danger); background: var(--danger-soft); }
.health-results > p { padding: 28px 14px; text-align: center; color: var(--text-2); }
.health-actions { grid-column: 2; grid-row: 1 / 3; align-self: center; justify-content: flex-end; }
@media (max-width: 680px) {
  article { grid-template-columns: minmax(0, 1fr); }
  .health-actions { grid-column: 1; grid-row: auto; justify-content: flex-start; }
  .health-toolbar label { margin-left: 0; }
}
summary { color: var(--text-2); font-size: 11px; }
[role="alert"] { color: var(--danger); }
summary {
  cursor: pointer;
}
label {
  display: flex;
  align-items: center;
  gap: 6px;
}
</style>
