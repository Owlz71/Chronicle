<script setup lang="ts">
import { Gamepad2, Sparkles, X } from "@lucide/vue";
import { onMounted, ref } from "vue";
import { createBackdropDismissal } from "../services/dialogDismissal";
import { t } from "../services/i18n";
import type { ExeCandidate } from "../services/saveSearch";

const props = defineProps<{
  candidates: ExeCandidate[];
  recommended: string | null;
  engineLabel: string;
}>();
const emit = defineEmits<{ cancel: []; select: [path: string] }>();
const selected = ref(props.recommended ?? props.candidates[0]?.path ?? "");
const closeButton = ref<HTMLButtonElement>();
const backdrop = createBackdropDismissal(() => emit("cancel"));
onMounted(() => closeButton.value?.focus());
</script>

<template>
  <div
    class="exe-backdrop"
    @pointerdown="backdrop.pointerDown"
    @pointerup="backdrop.pointerUp"
    @pointercancel="backdrop.pointerCancel"
  >
    <section
      class="exe-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="exe-title"
    >
      <header>
        <span><Gamepad2 :size="20" aria-hidden="true" /></span>
        <div>
          <p>{{ engineLabel || t("游戏存档识别") }}</p>
          <h2 id="exe-title">{{ t("找到多个可执行文件") }}</h2>
        </div>
        <button
          ref="closeButton"
          :aria-label="t('关闭程序选择')"
          :title="t('关闭程序选择')"
          @click="emit('cancel')"
        >
          <X :size="18" aria-hidden="true" />
        </button>
      </header>

      <p class="intro">
        {{
          t(
            "请选真正的游戏程序。启动器、崩溃处理程序和安装程序都不是游戏本体，选错会导致“游戏退出后”备份无法触发。",
          )
        }}
      </p>

      <ul class="candidates">
        <li v-for="candidate in candidates" :key="candidate.path">
          <label :class="{ selected: selected === candidate.path }">
            <input
              v-model="selected"
              type="radio"
              name="game-executable"
              :value="candidate.path"
            />
            <span>
              <b>{{ candidate.name }}</b>
              <small>{{ candidate.path }}</small>
            </span>
            <em v-if="candidate.likely" class="badge">
              <Sparkles :size="11" aria-hidden="true" />{{ t("推荐") }}
            </em>
          </label>
        </li>
      </ul>

      <footer>
        <button class="cancel" @click="emit('cancel')">{{ t("取消") }}</button>
        <button
          class="confirm"
          :disabled="!selected"
          @click="emit('select', selected)"
        >
          {{ t("继续") }}
        </button>
      </footer>
    </section>
  </div>
</template>

<style scoped>
.exe-backdrop {
  position: fixed;
  z-index: 83;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 24px;
  background: #18181b99;
  backdrop-filter: blur(3px);
}
.exe-dialog {
  display: flex;
  flex-direction: column;
  width: min(600px, calc(100vw - 48px));
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
.intro {
  margin: 0;
  padding: 16px 20px 10px;
  color: var(--text-2);
  font-size: 11px;
  line-height: 1.65;
}
.candidates {
  flex: 1;
  min-height: 0;
  margin: 0;
  padding: 4px 14px 16px;
  overflow-y: auto;
  list-style: none;
}
.candidates li + li {
  margin-top: 6px;
}
.candidates label {
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  padding: 11px 12px;
  color: var(--text-3);
  background: var(--subtle);
  border: 1px solid var(--border);
  border-radius: 9px;
  cursor: pointer;
}
.candidates label:hover {
  background: var(--hover);
  border-color: var(--border-2);
}
.candidates label.selected {
  color: var(--primary-dark);
  background: var(--primary-soft);
  border-color: color-mix(in srgb, var(--primary) 45%, var(--border));
}
.candidates label > span {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 3px;
}
.candidates b {
  color: var(--text);
  font-size: 11px;
}
.candidates label.selected b {
  color: var(--primary-dark);
}
.candidates small {
  overflow: hidden;
  color: var(--text-3);
  font-size: 9px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.badge {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 2px 7px;
  color: var(--on-primary);
  background: var(--primary);
  border-radius: 999px;
  font-size: 9px;
  font-style: normal;
  font-weight: 700;
}
input[type="radio"] {
  width: 14px;
  height: 14px;
  margin: 0;
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
