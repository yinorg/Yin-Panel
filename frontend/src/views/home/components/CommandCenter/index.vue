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
  padding: clamp(12vh, 18vh, 180px) var(--yin-pageGutter) 24px;
  background: color-mix(in srgb, var(--yin-canvas) 72%, transparent);
  backdrop-filter: blur(var(--yin-component-surface-blur));
}

.command-center-panel {
  width: min(720px, 100%);
  max-height: min(620px, 72vh);
  overflow: hidden;
  border: var(--yin-component-button-border-width) solid var(--yin-border);
  border-radius: var(--yin-component-dialog-radius);
  background: var(--yin-component-dialog-surface, var(--yin-surfaceElevated));
  box-shadow: var(--yin-component-dialog-shadow);
  color: var(--yin-text);
  font-family: var(--yin-fontBody);
}

.command-center-input-wrap {
  display: flex;
  align-items: center;
  gap: var(--yin-component-app-icon-gap);
  padding: var(--yin-component-dialog-padding);
  border-bottom: var(--yin-borderWidth) solid var(--yin-border);
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
  height: var(--yin-component-input-height);
  box-sizing: border-box;
  padding: 10px var(--yin-component-input-padding-x);
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--yin-text);
  font: var(--yin-fontBodyWeight) var(--yin-fontBodySize)/var(--yin-lineHeightBody) var(--yin-fontBody);
}

.command-center-input-wrap input::placeholder { color: var(--yin-textMuted); }

.command-center-submit-search {
  display: flex;
  flex: 0 0 25px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--yin-textMuted);
  cursor: pointer;
}

.command-center-submit-search:hover { color: var(--yin-primary); }

.command-center-results {
  max-height: min(520px, 58vh);
  overflow-y: auto;
  padding: var(--yin-component-menu-padding);
}

.command-center-result {
  display: flex;
  width: 100%;
  align-items: center;
  gap: var(--yin-component-group-gap);
  padding: var(--yin-component-menu-padding) var(--yin-component-dialog-padding);
  border: 0;
  border-radius: var(--yin-component-menu-radius);
  background: transparent;
  color: var(--yin-text);
  cursor: pointer;
  text-align: left;
}

.command-center-result.selected,
.command-center-result:hover { background: color-mix(in srgb, var(--yin-primary) 12%, transparent); }
.command-center-result-title { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.command-center-result-url { max-width: 48%; overflow: hidden; color: var(--yin-textMuted); font-size: var(--yin-fontSmallSize); text-overflow: ellipsis; white-space: nowrap; }
.command-center-empty { padding: var(--yin-component-dialog-padding); color: var(--yin-textMuted); text-align: center; }

@media (max-width: 600px) {
  .command-center-backdrop { padding: 10vh 10px 16px; }
  .command-center-result-url { display: none; }
}
</style>
