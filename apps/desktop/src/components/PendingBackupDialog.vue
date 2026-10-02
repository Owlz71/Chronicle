<script setup lang="ts">
import { AlertTriangle, ShieldAlert, X } from "@lucide/vue";
import { computed, onMounted, ref } from "vue";
import type { PendingChange } from "../services/backupHealth";
import { createBackdropDismissal } from "../services/dialogDismissal";
import { formatBytes, formatTime } from "../services/formatting";
import { t } from "../services/i18n";

const props = defineProps<{
  changes: PendingChange[];
  staleDays: number;
  busy: boolean;
}>();
const emit = defineEmits<{ cancel: []; confirm: [entryIds: string[]] }>();
const selected = ref(new Set(props.changes.map((change) => change.entryId)));
const closeButton = ref<HTMLButtonElement>();
const allSelected = computed(() => selected.value.size === props.changes.length);
const backdrop = createBackdropDismissal(
  () => emit("cancel"),
  () => !props.busy,
);

function toggle(entryId: string): void {
  const next = new Set(selected.value);
  if (!next.delete(entryId)) next.add(entryId);
  selected.value = next;
}

function toggleAll(): void {
  selected.value = allSelected.value
    ? new Set()
    : new Set(props.changes.map((change) => change.entryId));
}

onMounted(() => closeButton.value?.focus());
</script>

<template>
  <div
    class="pending-backdrop"
    @pointerdown="backdrop.pointerDown"
    @pointerup="backdrop.pointerUp"
    @pointercancel="backdrop.pointerCancel"
  >
    <section
      class="pending-dialog"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="pending-title"
    >
      <header>
        <span><ShieldAlert :size="20" aria-hidden="true" /></span>
        <div>
          <p>{{ t("同步前的检查") }}</p>
          <h2 id="pending-title">{{ t("有存档很久没有备份") }}</h2>
        </div>
        <button
          ref="closeButton"
          :aria-label="t('关闭同步前检查')"
          :title="t('关闭同步前检查')"
          :disabled="busy"
          @click="emit('cancel')"
        >
          <X :size="18" aria-hidden="true" />
        </button>
      </header>

      <p class="intro">
        {{
          t(
            "这些存档的内容有变化，但已经超过 {days} 天没有创建新的时间节点。先备份再同步，可以避免把变化推到云端却没有本机回退点。",
            { days: staleDays },
          )
        }}
      </p>

      <label class="select-all">
        <input
          type="checkbox"
          :checked="allSelected"
          :disabled="busy"
          @change="toggleAll"
        />
        <span>{{ t("全选（{count} 个）", { count: changes.length }) }}</span>
      </label>

      <ul class="changes">
        <li v-for="change in changes" :key="change.entryId">
          <label>
            <input
              type="checkbox"
              :checked="selected.has(change.entryId)"
              :disabled="busy"
              @change="toggle(change.entryId)"
            />
            <span>
              <b>{{ change.name }}</b>
              <small>{{
                t("上次备份 {time} · {size}", {
                  time: formatTime(change.lastSnapshotAt ?? undefined),
                  size: formatBytes(change.totalBytes),
                })
              }}</small>
              <small v-if="change.unreadableSources" class="warning">
                <AlertTriangle :size="12" aria-hidden="true" />{{
                  t("{count} 个来源无法读取，已跳过比对", {
                    count: change.unreadableSources,
                  })
                }}
              </small>
            </span>
          </label>
        </li>
      </ul>

      <footer>
        <button class="cancel" :disabled="busy" @click="emit('cancel')">
          {{ t("取消") }}
        </button>
        <button
          class="confirm"
          :disabled="busy || !selected.size"
          @click="emit('confirm', [...selected])"
        >
          {{
            busy
              ? t("处理中")
              : t("备份并同步（{count}）", { count: selected.size })
          }}
        </button>
      </footer>
    </section>
  </div>
</template>

<style scoped>
.pending-backdrop {
  position: fixed;
  z-index: 85;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 24px;
  background: #18181b99;
  backdrop-filter: blur(3px);
}
.pending-dialog {
  display: flex;
  flex-direction: column;
  width: min(560px, calc(100vw - 48px));
  max-height: min(660px, calc(100vh - 80px));
  overflow: hidden;
  color: var(--text);
  background: var(--surface);
  border: 1px solid var(--border-2);
  border-radius: 12px;
  box-shadow: 0 24px 80px var(--shadow-color);
}
header {
  display: grid;
  grid-template-columns: 40px 1fr 38px;
  align-items: center;
  gap: 11px;
  padding: 17px 18px;
  border-bottom: 1px solid var(--border);
}
header > span {
  display: grid;
  place-items: center;
  width: 38px;
  height: 38px;
  color: var(--warning);
  background: var(--warning-soft);
  border-radius: 9px;
}
header p,
header h2 {
  margin: 0;
}
header p {
  color: var(--text-3);
  font-size: 9px;
  font-weight: 700;
}
header h2 {
  margin-top: 3px;
  font-size: 17px;
}
header button {
  display: grid;
  place-items: center;
  width: 38px;
  height: 38px;
  color: var(--text-2);
  background: transparent;
  border-radius: 7px;
}
header button:hover {
  color: var(--text);
  background: var(--hover);
}
.intro {
  margin: 0;
  padding: 16px 20px 12px;
  color: var(--text-2);
  font-size: 11px;
  line-height: 1.65;
}
.select-all {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 20px 6px;
  padding: 8px 10px;
  color: var(--text-2);
  background: var(--subtle);
  border-radius: 7px;
  font-size: 10px;
  font-weight: 650;
}
.changes {
  flex: 1;
  min-height: 0;
  margin: 0;
  padding: 4px 20px 16px;
  overflow-y: auto;
  list-style: none;
}
.changes li {
  border-bottom: 1px solid var(--border);
}
.changes li:last-child {
  border-bottom: 0;
}
.changes label {
  display: flex;
  align-items: flex-start;
  gap: 9px;
  padding: 10px 2px;
  cursor: pointer;
}
.changes label:hover {
  background: var(--hover);
}
.changes label > span {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 3px;
}
.changes b {
  color: var(--text);
  font-size: 11px;
}
.changes small {
  color: var(--text-3);
  font-size: 9px;
}
.changes small.warning {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--warning);
}
input[type="checkbox"] {
  flex: none;
  width: 15px;
  height: 15px;
  margin: 1px 0 0;
  accent-color: var(--primary);
  cursor: pointer;
}
footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 13px 18px;
  background: var(--subtle);
  border-top: 1px solid var(--border);
}
footer button {
  min-height: 36px;
  padding: 0 14px;
  font-size: 11px;
  font-weight: 650;
  border-radius: 7px;
}
.cancel {
  color: var(--text-2);
  background: var(--surface);
  border: 1px solid var(--border-2);
}
.cancel:hover {
  color: var(--text);
  background: var(--hover);
}
.confirm {
  color: var(--on-primary);
  background: var(--primary);
}
.confirm:hover:not(:disabled) {
  background: var(--primary-dark);
}
button:disabled {
  cursor: default;
  opacity: 0.55;
}
</style>
