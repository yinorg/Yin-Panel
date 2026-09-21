<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { NAvatar } from 'naive-ui'
import SvgIcon from '@/components/common/SvgIcon/index.vue'
import type { SearchEngine } from '@/components/deskModule/SearchBox/engines'

interface CommandDefinition {
  key: string
  label: string
}

const props = defineProps<{
  visible: boolean
  query: string
  items: Panel.ItemInfo[]
  commands: CommandDefinition[]
  selectedIndex: number
  searchEngine: SearchEngine
}>()

const emit = defineEmits<{
  (e: 'update:query', value: string): void
  (e: 'move', offset: number): void
  (e: 'select', index: number): void
  (e: 'submit-search', query: string): void
  (e: 'execute-item', item: Panel.ItemInfo): void
  (e: 'execute-command', command: string): void
  (e: 'close'): void
}>()

const inputRef = ref<HTMLInputElement | null>(null)
const isCommandQuery = computed(() => props.query.startsWith('/'))

watch(() => props.visible, (visible) => {
  if (visible) nextTick(() => inputRef.value?.focus())
})

function handleInput(event: Event) {
  const value = (event.target as HTMLInputElement).value
  emit('update:query', value)
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'ArrowDown') {
    event.preventDefault()
    emit('move', 1)
  }
  else if (event.key === 'ArrowUp') {
    event.preventDefault()
    emit('move', -1)
  }
  else if (event.key === 'Enter') {
    event.preventDefault()
    if (isCommandQuery.value) {
      const command = props.commands[props.selectedIndex]
      if (command) emit('execute-command', command.key)
    }
    else {
      const item = props.items[props.selectedIndex]
      if (item) emit('execute-item', item)
      else emit('submit-search', props.query)
    }
  }
  else if (event.key === 'Escape') {
    event.preventDefault()
    emit('close')
  }
}

function submitSearch() {
  emit('submit-search', props.query)
}
</script>

<template>
  <div v-if="visible" data-testid="command-center-backdrop" class="command-center-backdrop" @click.self="emit('close')">
    <section data-testid="command-center-panel" class="command-center-panel" role="dialog" aria-modal="true" @click.stop>
      <div class="command-center-input-wrap">
        <span data-testid="command-center-search-engine" class="command-center-engine" :title="searchEngine.title">
          <NAvatar :src="searchEngine.iconSrc" style="background-color: transparent;" :size="20" />
        </span>
        <input
          ref="inputRef"
          data-testid="command-center-input"
          :value="query"
          :placeholder="$t('deskModule.searchBox.inputPlaceholder')"
          autocomplete="off"
          spellcheck="false"
          @input="handleInput"
          @keydown="handleKeydown"
        >
        <button
          v-if="!isCommandQuery"
          data-testid="command-center-submit-search"
          type="button"
          class="command-center-submit-search"
          :title="$t('deskModule.searchBox.inputPlaceholder')"
          @click="submitSearch"
        >
          <SvgIcon style="width: 20px;height: 20px;" icon="iconamoon:search-fill" />
        </button>
      </div>

      <div class="command-center-results">
        <button
          v-for="(item, index) in items"
          v-show="!isCommandQuery"
          :key="`item-${item.id || index}`"
          type="button"
          class="command-center-result"
          :class="{ selected: index === selectedIndex }"
          @mouseenter="emit('select', index)"
          @click="emit('execute-item', item)"
        >
          <span class="command-center-result-title">{{ item.title }}</span>
          <span class="command-center-result-url">{{ item.url }}</span>
        </button>

        <button
          v-for="(command, index) in commands"
          v-show="isCommandQuery"
          :key="command.key"
          type="button"
          class="command-center-result"
          :class="{ selected: index === selectedIndex }"
          @mouseenter="emit('select', index)"
          @click="emit('execute-command', command.key)"
        >
          <span class="command-center-result-title">{{ command.label }}</span>
          <span class="command-center-result-url">/{{ command.key }}</span>
        </button>

        <div v-if="(isCommandQuery && !commands.length) || (!isCommandQuery && query && !items.length)" class="command-center-empty">
          {{ $t('common.noData') }}
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.command-center-backdrop {
  position: fixed;
  z-index: 1000;
  inset: 0;
  display: flex;
  justify-content: center;
  align-items: flex-start;
  padding: clamp(12vh, 18vh, 180px) 16px 24px;
  background: rgba(8, 10, 14, 0.58);
  backdrop-filter: blur(4px);
}

.command-center-panel {
  width: min(720px, 100%);
  max-height: min(620px, 72vh);
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 14px;
  background: rgba(25, 29, 36, 0.94);
  box-shadow: 0 24px 80px rgba(0, 0, 0, 0.45);
  color: white;
}

.command-center-input-wrap {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px 18px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.12);
}

.command-center-engine {
  display: flex;
  flex: 0 0 40px;
  align-items: center;
  justify-content: center;
}

.command-center-input-wrap input {
  width: 100%;
  min-width: 0;
  height: 40px;
  box-sizing: border-box;
  padding: 10px 5px;
  border: 0;
  outline: 0;
  background: transparent;
  color: white;
  font-size: 17px;
}

.command-center-input-wrap input::placeholder { color: rgba(255, 255, 255, 0.46); }

.command-center-submit-search {
  display: flex;
  flex: 0 0 25px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  background: transparent;
  color: rgba(255, 255, 255, 0.76);
  cursor: pointer;
}

.command-center-submit-search:hover { color: white; }

.command-center-results {
  max-height: min(520px, 58vh);
  overflow-y: auto;
  padding: 8px;
}

.command-center-result {
  display: flex;
  width: 100%;
  align-items: center;
  gap: 14px;
  padding: 12px 14px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: rgba(255, 255, 255, 0.88);
  cursor: pointer;
  text-align: left;
}

.command-center-result.selected,
.command-center-result:hover { background: rgba(255, 255, 255, 0.12); }
.command-center-result-title { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.command-center-result-url { max-width: 48%; overflow: hidden; color: rgba(255, 255, 255, 0.48); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
.command-center-empty { padding: 28px 14px; color: rgba(255, 255, 255, 0.54); text-align: center; }

@media (max-width: 600px) {
  .command-center-backdrop { padding: 10vh 10px 16px; }
  .command-center-result-url { display: none; }
}
</style>
