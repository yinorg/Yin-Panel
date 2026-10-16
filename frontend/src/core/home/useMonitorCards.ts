import { onMounted, ref } from 'vue'
import type { ThemeMonitorCard } from '@/theme/api/v1'

const CARD_TYPES = new Set(['cpu', 'memory', 'disk', 'network'])

/**
 * The visitor's monitor cards, in the order they arranged them.
 *
 * The Core band reads these from the module config, which is a cloud value, so
 * they cannot be derived from the panel config the snapshot is otherwise built
 * from — they have to be fetched. A theme that draws its own band renders exactly
 * these cards instead of guessing a default set.
 *
 * The reader is imported lazily: it reaches the module-config store at module
 * scope, and that module is normally loaded only with the monitor component, so
 * importing it eagerly here would run it before the app is ready.
 */
export function useMonitorCards(enabled: () => boolean = () => true) {
  const cards = ref<ThemeMonitorCard[] | undefined>(undefined)

  onMounted(() => {
    // A public link may not read the module config: the endpoint is JWT-only and the
    // theme's band is hidden there anyway, so the read only earns a refusal.
    if (!enabled())
      return
    void import('@/components/deskModule/SystemMonitor/common').then(({ getAll }) => getAll()).then((list) => {
      cards.value = list.flatMap((card) => {
        if (!CARD_TYPES.has(card.monitorType))
          return []
        const extend = (card.extendParam || {}) as Record<string, unknown>
        const text = (key: string) => typeof extend[key] === 'string' ? extend[key] as string : undefined
        return [{
          type: card.monitorType,
          path: text('path'),
          backgroundColor: text('backgroundColor'),
          color: text('color'),
          progressColor: text('progressColor'),
          progressRailColor: text('progressRailColor'),
        }]
      })
    }).catch(() => { cards.value = undefined })
  })

  return { monitorCards: cards }
}
