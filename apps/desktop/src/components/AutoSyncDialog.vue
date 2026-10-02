<script setup lang="ts">
import { CloudOff, Timer, X } from "@lucide/vue";
import { computed, onMounted, ref } from "vue";
import { createBackdropDismissal } from "../services/dialogDismissal";
import { formatTime } from "../services/formatting";
import { t } from "../services/i18n";

const props = defineProps<{
  enabled: boolean;
  intervalDays: number;
  lastSyncAt: number;
  cloudReady: boolean;
  syncing: boolean;
  saving?: boolean;
}>();
const emit = defineEmits<{
  close: [];
  save: [payload: { enabled: boolean; intervalDays: number; syncNow: boolean }];
}>();
const enabled = ref(props.enabled);
const intervalDays = ref(props.intervalDays);
const closeButton = ref<HTMLButtonElement>();
const backdrop = createBackdropDismissal(
  () => emit("close"),
  () => !props.saving,
);
const invalid = computed(
  () =>
    !Number.isFinite(intervalDays.value) ||
    intervalDays.value < 1 ||
    intervalDays.value > 365,
);
const needsCloud = computed(() => enabled.value && !props.cloudReady);
const nextSyncAt = computed(() =>
  props.lastSyncAt
    ? props.lastSyncAt + intervalDays.value * 86_400_000
    : undefined,
);

function submit(syncNow: boolean): void {
  if (invalid.value || needsCloud.value) return;
  emit("save", {
    enabled: enabled.value,
    intervalDays: Math.floor(intervalDays.value),
    syncNow,
  });
}

onMounted(() => closeButton.value?.focus());
</script>

<template>
  <div
    class="auto-sync-backdrop"
    @pointerdown="backdrop.pointerDown"
    @pointerup="backdrop.pointerUp"
    @pointercancel="backdrop.pointerCancel"
  >
    <section
      class="auto-sync-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="auto-sync-title"
    >
      <header>
        <span><Timer :size="20" aria-hidden="true" /></span>
        <div>
          <p>{{ t("云端同步") }}</p>
          <h2 id="auto-sync-title">{{ t("自动同步间隔") }}</h2>
        </div>
        <button
          ref="closeButton"
          :aria-label="t('关闭自动同步设置')"
          :title="t('关闭自动同步设置')"
          :disabled="saving"
          @click="emit('close')"
        >
          <X :size="18" aria-hidden="true" />
        </button>
      </header>

      <div class="body">
        <label class="toggle-row">
          <span>
            <b>{{ t("开启自动同步") }}</b>
            <small>{{
              t("按下面的间隔检查一次，到时间才同步；程序启动时也会检查。重新保存设置不会重置计时。")
            }}</small>
          </span>
          <input v-model="enabled" type="checkbox" role="switch" />
        </label>

        <label class="interval-row">
          <span>{{ t("间隔（天）") }}</span>
          <input
            v-model.number="intervalDays"
            type="number"
            min="1"
            max="365"
            :aria-invalid="invalid"
          />
        </label>
        <small v-if="invalid" class="error" role="alert">{{
          t("间隔必须是 1–365 之间的天数")
        }}</small>

        <dl v-if="enabled">
          <div>
            <dt>{{ t("上次同步") }}</dt>
            <dd>{{ lastSyncAt ? formatTime(lastSyncAt) : t("尚未同步") }}</dd>
          </div>
          <div>
            <dt>{{ t("下次自动同步") }}</dt>
            <dd>{{ nextSyncAt ? formatTime(nextSyncAt) : t("尚未确定") }}</dd>
          </div>
        </dl>

        <p v-if="needsCloud" class="warning" role="alert">
          <CloudOff :size="15" aria-hidden="true" />{{
            t("还没有可用的云端同步源，请先在云端设置里添加并启用。")
          }}
        </p>
        <p class="hint">
          {{
            t(
              "同步前会检查有没有超过提醒天数却没有备份的存档，有的话会先询问你是否备份。",
            )
          }}
        </p>
      </div>

      <footer>
        <button class="cancel" :disabled="saving" @click="emit('close')">
          {{ t("取消") }}
        </button>
        <button
          class="now"
          :disabled="saving || syncing || invalid"
          @click="submit(true)"
        >
          {{ syncing ? t("同步中") : t("立即同步一次") }}
        </button>
        <button
          class="save"
          :disabled="saving || invalid || needsCloud"
          @click="submit(false)"
        >
          {{ saving ? t("保存中") : t("保存设置") }}
        </button>
      </footer>
    </section>
  </div>
</template>

<style scoped>
.auto-sync-backdrop {
  position: fixed;
  z-index: 84;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 24px;
  background: #18181b99;
  backdrop-filter: blur(3px);
}
.auto-sync-dialog {
  display: flex;
  flex-direction: column;
  width: min(470px, calc(100vw - 48px));
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
  color: var(--primary);
  background: var(--primary-soft);
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
.body {
  padding: 18px 20px 8px;
}
.toggle-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  min-height: 60px;
  padding: 10px 14px;
  background: var(--subtle);
  border-radius: 8px;
}
.toggle-row > span {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.toggle-row b {
  font-size: 11px;
}
.toggle-row small,
.hint {
  color: var(--text-3);
  font-size: 9px;
  line-height: 1.5;
}
.toggle-row input {
  position: relative;
  width: 38px;
  height: 22px;
  flex: none;
  appearance: none;
  background: var(--border-2);
  border-radius: 20px;
  cursor: pointer;
}
.toggle-row input::after {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 16px;
  height: 16px;
  content: "";
  background: var(--surface);
  border-radius: 50%;
  box-shadow: 0 1px 3px var(--shadow-color);
  transition: transform 0.16s;
}
.toggle-row input:checked {
  background: var(--primary);
}
.toggle-row input:checked::after {
  transform: translateX(16px);
}
.interval-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  margin-top: 14px;
}
.interval-row > span {
  color: var(--text-2);
  font-size: 11px;
  font-weight: 650;
}
.interval-row input {
  width: 104px;
  height: 38px;
  padding: 0 11px;
  color: var(--text);
  background: var(--field);
  border: 1px solid var(--border-2);
  border-radius: 7px;
  font-size: 12px;
}
.interval-row input[aria-invalid="true"] {
  border-color: var(--danger);
}
.error {
  display: block;
  margin-top: 7px;
  color: var(--danger);
  font-size: 9px;
}
dl {
  margin: 16px 0 0;
}
dl > div {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 0;
  border-top: 1px solid var(--border);
  font-size: 10px;
}
dt {
  color: var(--text-3);
}
dd {
  margin: 0;
  color: var(--text-2);
  font-variant-numeric: tabular-nums;
}
.warning {
  display: flex;
  align-items: flex-start;
  gap: 7px;
  margin: 14px 0 0;
  padding: 10px 12px;
  color: var(--warning);
  background: var(--warning-soft);
  border: 1px solid var(--warning-border);
  border-radius: 7px;
  font-size: 10px;
  line-height: 1.5;
}
.hint {
  margin: 14px 0 0;
}
footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 12px;
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
.now {
  color: var(--text-2);
  background: var(--surface);
  border: 1px solid var(--border-2);
}
.now:hover:not(:disabled) {
  color: var(--text);
  background: var(--hover);
}
.save {
  color: var(--on-primary);
  background: var(--primary);
}
.save:hover:not(:disabled) {
  background: var(--primary-dark);
}
button:disabled {
  cursor: default;
  opacity: 0.55;
}
</style>
