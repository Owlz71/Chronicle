<script setup lang="ts">
import { AlertCircle, File, Folder, FolderSearch, Search, Sparkles, X } from "@lucide/vue";
import { computed, onMounted, ref } from "vue";
import { createBackdropDismissal } from "../services/dialogDismissal";
import { t } from "../services/i18n";
import { executableLabel, type SaveSearchHit } from "../services/saveSearch";

const props = defineProps<{
  executablePath: string;
  hits: SaveSearchHit[];
  busy: boolean;
  error?: string;
  /** Engine name shown above the title when detection found one. */
  engine?: string;
}>();
const emit = defineEmits<{ close: []; select: [hit: SaveSearchHit] }>();
const closeButton = ref<HTMLButtonElement>();
const exactCount = computed(
  () => props.hits.filter((hit) => hit.exact).length,
);
const backdrop = createBackdropDismissal(
  () => emit("close"),
  () => !props.busy,
);
onMounted(() => closeButton.value?.focus());
</script>

<template>
  <div
    class="save-search-backdrop"
    @pointerdown="backdrop.pointerDown"
    @pointerup="backdrop.pointerUp"
    @pointercancel="backdrop.pointerCancel"
  >
    <section
      class="save-search-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="save-search-title"
    >
      <header>
        <span><FolderSearch :size="20" aria-hidden="true" /></span>
        <div>
          <p>{{ engine || t("在 AppData 中搜索存档") }}</p>
          <h2 id="save-search-title">{{ executableLabel(executablePath) }}</h2>
        </div>
        <button
          ref="closeButton"
          :aria-label="t('关闭搜索结果')"
          :title="t('关闭搜索结果')"
          :disabled="busy"
          @click="emit('close')"
        >
          <X :size="18" aria-hidden="true" />
        </button>
      </header>

      <p v-if="busy" class="state">
        <Search :size="15" aria-hidden="true" />{{ t("正在搜索 AppData…") }}
      </p>
      <p v-else-if="error" class="state error" role="alert">
        <AlertCircle :size="15" aria-hidden="true" />{{ error }}
      </p>
      <p v-else-if="!hits.length" class="state">
        {{ t("没有找到匹配的存档位置，可以关闭后手动选择。") }}
      </p>
      <template v-else>
        <p class="summary">
          {{
            t("找到 {count} 个可能的存档文件夹，其中 {exact} 个完全匹配。", {
              count: hits.length,
              exact: exactCount,
            })
          }}
        </p>
        <ul class="hits">
          <li v-for="hit in hits" :key="hit.path">
            <button
              class="hit"
              :class="{ exact: hit.exact }"
              @click="emit('select', hit)"
            >
              <span class="hit-name">
                <Folder v-if="hit.kind === 'folder'" :size="15" aria-hidden="true" />
                <File v-else :size="15" aria-hidden="true" />
                <strong>{{ hit.name }}</strong>
                <em v-if="hit.exact" class="badge">
                  <Sparkles :size="11" aria-hidden="true" />{{
                    t("完全匹配")
                  }}</em
                >
              </span>
              <small>{{ hit.path }}</small>
            </button>
          </li>
        </ul>
      </template>

      <footer>
        <button class="cancel" :disabled="busy" @click="emit('close')">
          {{ t("取消") }}
        </button>
      </footer>
    </section>
  </div>
</template>

<style scoped>
.save-search-backdrop {
  position: fixed;
  z-index: 82;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 24px;
  background: #18181b99;
  backdrop-filter: blur(3px);
}
.save-search-dialog {
  display: flex;
  flex-direction: column;
  width: min(620px, calc(100vw - 48px));
  max-height: min(640px, calc(100vh - 80px));
  overflow: hidden;
  background: var(--surface);
  border: 1px solid var(--border-2);
  border-radius: 12px;
  box-shadow: 0 24px 80px var(--shadow-color);
  color: var(--text);
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
  overflow: hidden;
  color: var(--text);
  font-size: 17px;
  text-overflow: ellipsis;
  white-space: nowrap;
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
.state,
.summary {
  display: flex;
  align-items: center;
  gap: 7px;
  margin: 0;
  padding: 16px 20px;
  color: var(--text-2);
  font-size: 12px;
}
.state.error {
  color: var(--danger);
}
.summary {
  padding-bottom: 8px;
}
.hits {
  flex: 1;
  min-height: 0;
  margin: 0;
  padding: 4px 12px 14px;
  overflow-y: auto;
  list-style: none;
}
.hits li + li {
  margin-top: 6px;
}
.hit {
  display: flex;
  flex-direction: column;
  gap: 3px;
  width: 100%;
  padding: 10px 12px;
  color: var(--text);
  text-align: left;
  background: var(--subtle);
  border: 1px solid var(--border);
  border-radius: 9px;
}
.hit:hover {
  background: var(--hover);
  border-color: var(--border-2);
}
.hit.exact {
  background: var(--primary-soft);
  border-color: color-mix(in srgb, var(--primary) 45%, var(--border));
  box-shadow: inset 3px 0 0 var(--primary);
}
.hit-name {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text);
  font-size: 12px;
}
.hit-name svg {
  flex: none;
  color: var(--text-3);
}
.badge {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 2px 7px;
  font-size: 9px;
  font-style: normal;
  font-weight: 700;
  color: var(--on-primary);
  background: var(--primary);
  border-radius: 999px;
}
.hit small {
  overflow: hidden;
  color: var(--text-3);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
footer {
  display: flex;
  justify-content: flex-end;
  padding: 13px 18px;
  background: var(--subtle);
  border-top: 1px solid var(--border);
}
footer button {
  min-height: 36px;
  padding: 0 14px;
  font-size: 11px;
  font-weight: 650;
  border: 1px solid transparent;
  border-radius: 7px;
}
.cancel {
  color: var(--text-2);
  background: var(--surface);
  border-color: var(--border-2);
}
.cancel:hover {
  color: var(--text);
  background: var(--hover);
}
</style>
