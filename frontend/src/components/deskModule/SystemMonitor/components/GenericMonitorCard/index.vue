<script setup lang="ts">
// -------------------
// 系统监控图标临时使用（与 AppIcon/index.vue 一致）
// 如果确定这种方案将 AppIcon/index.vue 封装成通用组件
// -------------------

import { computed, ref } from 'vue'
import { ItemCard, SvgIcon } from '../../../../common'
import type { PanelPanelConfigStyleEnum } from '../../../../../enums'

interface Prop {
//   size?: number // 默认70
  extendParam?: any
  iconTextColor?: string
  iconTextIconHideTitle?: boolean
  iconText?: string
  textColor?: string
  cardTypeStyle: PanelPanelConfigStyleEnum
  // monitorType: string
  icon?: string
  class?: string
  backgroundColor?: string
}

const props = withDefaults(defineProps<Prop>(), {})
const propClass = ref(props.class)

const customFill = computed(() => {
  const value = (props.backgroundColor || '').trim()
  return value !== '' && !value.includes('var(')
})

const derivedTextColor = computed(() => {
  if (!customFill.value) return undefined
  const parsed = parseColor(props.backgroundColor!)
  if (!parsed) return undefined
  // A translucent fill is composited over the mid grey a wallpaper averages to,
  // which is close enough to decide black-or-white.
  const base = parsed.a >= 0.999 ? parsed : composite(parsed, { r: 128, g: 128, b: 128, a: 1 })
  return relativeLuminance(base) < 0.35 ? '#ffffff' : '#101418'
})

const effectiveTextColor = computed(() => derivedTextColor.value || props.textColor || 'var(--yin-text)')

function parseColor(value: string) {
  const hex = value.match(/^#([0-9a-f]{3,8})$/i)
  if (hex) {
    const digits = hex[1]
    const expand = (part: string) => Number.parseInt(part.length === 1 ? part + part : part, 16)
    if (digits.length === 3 || digits.length === 4)
      return { r: expand(digits[0]), g: expand(digits[1]), b: expand(digits[2]), a: digits.length === 4 ? expand(digits[3]) / 255 : 1 }
    if (digits.length === 6 || digits.length === 8)
      return { r: expand(digits.slice(0, 2)), g: expand(digits.slice(2, 4)), b: expand(digits.slice(4, 6)), a: digits.length === 8 ? expand(digits.slice(6, 8)) / 255 : 1 }
    return undefined
  }
  const rgb = value.match(/^rgba?\(([^)]+)\)$/i)
  if (!rgb) return undefined
  const parts = rgb[1].split(/[,\s/]+/).filter(Boolean).map(Number)
  if (parts.length < 3 || parts.some(Number.isNaN)) return undefined
  return { r: parts[0], g: parts[1], b: parts[2], a: parts.length > 3 ? parts[3] : 1 }
}

function composite(foreground: { r: number, g: number, b: number, a: number }, background: { r: number, g: number, b: number, a: number }) {
  return {
    r: foreground.r * foreground.a + background.r * (1 - foreground.a),
    g: foreground.g * foreground.a + background.g * (1 - foreground.a),
    b: foreground.b * foreground.a + background.b * (1 - foreground.a),
    a: 1,
  }
}

function relativeLuminance(color: { r: number, g: number, b: number }) {
  const channel = (value: number) => {
    const scaled = value / 255
    return scaled <= 0.03928 ? scaled / 12.92 : ((scaled + 0.055) / 1.055) ** 2.4
  }
  return 0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}
</script>

<template>
  <div class="generic-monitor-card w-full">
    <ItemCard
      :card-type-style="cardTypeStyle"
      :icon-text="iconText"
      :icon-text-color="effectiveTextColor"
      :class="propClass"
      :background-color="backgroundColor"
      :icon-text-icon-hide-title="iconTextIconHideTitle"
    >
      <template #info>
        <!-- 图标 -->
        <div class="w-[60px] h-[70px]">
          <div class="w-[60px] h-full flex items-center justify-center" :style="{ color: effectiveTextColor }">
            <slot name="icon">
              <SvgIcon :icon="icon ?? ''" :style="{ color: effectiveTextColor, width: 'var(--yin-component-system-monitor-icon-size)', height: 'var(--yin-component-system-monitor-icon-size)' }" />
            </slot>
          </div>
        </div>

        <div
          class=" w-full flex items-center"
          :style="{ maxWidth: 'calc(100% - 80px)', color: effectiveTextColor }"
        >
          <slot name="info" />
        </div>
      </template>
      <template #small>
        <slot name="small" />
      </template>
    </ItemCard>
  </div>
</template>
