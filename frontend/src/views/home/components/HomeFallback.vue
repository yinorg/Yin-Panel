<script setup lang="ts">
/**
 * Minimum viable home view — boundary C3 of the architecture (see PLAN_V5 §2.1).
 *
 * This is not a restyled copy of the Core home. It exists so that "the theme
 * cannot render the home" is never a white screen, which becomes load-bearing
 * once P4a removes the Core's own presentation. It therefore depends on nothing
 * from the theme and nothing from the Core's presentation layer:
 *
 * - styles are scoped and hard-coded, so a theme that ships broken or missing CSS
 *   custom properties cannot make this unreadable;
 * - one flat list of spaces, groups and items, so there is no layout contract to
 *   break and no interaction beyond "open this bookmark" and "switch space";
 * - no Naive UI components, no icon rendering, no wallpaper.
 *
 * Do not grow features here. Anything richer belongs in the theme; the fallback
 * only has to stay legible and let the user reach their links and the recovery
 * page.
 */
import { computed } from 'vue'
import type { Space } from '@/api/panel/space'

// Ids stay optional: the Core's collection records carry optional ids, and this
// view must accept them as-is rather than force a lossy re-projection.
interface FallbackItem {
  id?: number | string
  title?: string
  description?: string
}

interface FallbackGroup {
  id?: number | string
  title?: string
  items?: readonly FallbackItem[]
}

const props = defineProps<{
  spaces: readonly Space[]
  activeSpaceId?: number
  groups: readonly FallbackGroup[]
  /** Already-localised explanation of why the theme is not rendering. */
  reason: string
  /** Brand text from the panel configuration, so the fallback shows the user's
   *  own name for the instance instead of a hard-coded one. */
  brand: string
}>()

// Only ids cross this boundary. The fallback is a degraded view with no authority
// over item data, so it reports *which* bookmark was chosen and lets the Core
// resolve and open it exactly as the full home would.
const emit = defineEmits<{
  (event: 'select-space', spaceId: number): void
  (event: 'open-item', itemId?: number | string): void
  (event: 'refresh'): void
}>()

const visibleGroups = computed(() => props.groups.filter(group => (group.items || []).length > 0))
</script>

<template>
  <div class="fallback-home" data-testid="home-fallback">
    <div class="fallback-panel">
      <p class="fallback-heading" data-lcp="brand">
        {{ props.brand }}
      </p>
      <p class="fallback-notice" data-testid="home-fallback-notice">
        {{ $t('panelHome.fallbackNotice') }}
        <span class="fallback-reason">{{ $t('panelHome.fallbackReason') }}: {{ props.reason }}</span>
      </p>

      <div v-if="props.spaces.length > 1" class="fallback-spaces">
        <button
          v-for="space in props.spaces"
          :key="space.id"
          type="button"
          class="fallback-space"
          :class="{ 'fallback-space--active': space.id === props.activeSpaceId }"
          :aria-pressed="space.id === props.activeSpaceId"
          :data-testid="`home-fallback-space-${space.id}`"
          @click="emit('select-space', space.id)"
        >
          {{ space.name }}
        </button>
      </div>

      <p v-if="visibleGroups.length === 0" class="fallback-empty" data-testid="home-fallback-empty">
        {{ $t('panelHome.fallbackEmpty') }}
      </p>
      <section v-for="group in visibleGroups" :key="group.id" class="fallback-group" data-testid="home-fallback-group">
        <h2 class="fallback-group-title">
          {{ group.title }}
        </h2>
        <ul class="fallback-items">
          <li v-for="item in group.items" :key="item.id">
            <button
              type="button"
              class="fallback-item"
              data-testid="home-fallback-item"
              @click="emit('open-item', item.id)"
            >
              <span class="fallback-item-title">{{ item.title }}</span>
              <span v-if="item.description" class="fallback-item-description">{{ item.description }}</span>
            </button>
          </li>
        </ul>
      </section>

      <p class="fallback-actions">
        <button type="button" class="fallback-action" data-testid="home-fallback-refresh" @click="emit('refresh')">
          {{ $t('panelHome.fallbackReload') }}
        </button>
        <a class="fallback-action" href="/theme-recovery.html" data-testid="home-fallback-recovery">{{ $t('themeRecovery.title') }}</a>
      </p>
    </div>
  </div>
</template>

<style scoped>
/* Deliberately hard-coded colours and lengths: no theme token, no Core design
   token, no Tailwind class. If those layers are broken, this must still read. */
.fallback-home {
  position: absolute;
  inset: 0;
  z-index: 2;
  overflow: auto;
  padding: 24px 16px;
  background: #0f1216;
  color: #e8eaed;
  font-family: system-ui, -apple-system, 'Segoe UI', sans-serif;
  font-size: 15px;
  line-height: 1.5;
}

.fallback-panel {
  max-width: 720px;
  margin: 0 auto;
}

.fallback-heading {
  margin: 0 0 4px;
  font-size: 28px;
  font-weight: 700;
}

.fallback-notice {
  margin: 0 0 24px;
  color: #9aa4ae;
}

.fallback-reason {
  display: block;
  margin-top: 4px;
  font-size: 13px;
  word-break: break-word;
}

.fallback-spaces {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 24px;
}

.fallback-space {
  padding: 6px 14px;
  border: 1px solid #2f3a44;
  border-radius: 999px;
  background: #171c22;
  color: inherit;
  font: inherit;
  cursor: pointer;
}

.fallback-space--active {
  border-color: #7dd3fc;
  color: #7dd3fc;
}

.fallback-group {
  margin-bottom: 24px;
}

.fallback-group-title {
  margin: 0 0 8px;
  font-size: 16px;
  font-weight: 600;
}

.fallback-items {
  margin: 0;
  padding: 0;
  list-style: none;
}

.fallback-item {
  display: block;
  width: 100%;
  padding: 8px 10px;
  border: 1px solid transparent;
  border-bottom-color: #1d252d;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.fallback-item:hover,
.fallback-item:focus-visible {
  border-color: #2f3a44;
  background: #171c22;
}

.fallback-item-title {
  display: block;
}

.fallback-item-description {
  display: block;
  color: #9aa4ae;
  font-size: 13px;
}

.fallback-empty {
  color: #9aa4ae;
}

.fallback-actions {
  display: flex;
  gap: 12px;
  margin: 32px 0 0;
}

.fallback-action {
  padding: 8px 16px;
  border: 1px solid #2f3a44;
  border-radius: 8px;
  background: #171c22;
  color: inherit;
  font: inherit;
  text-decoration: none;
  cursor: pointer;
}
</style>
