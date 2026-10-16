<script setup lang="ts">
import { NAvatar, useThemeVars } from 'naive-ui'
import { computed, ref, watch } from 'vue'
import { SvgIconOnline } from '../index'
import { normalizeIconBackground, readableTextColor } from '../../../utils/color'

interface Prop {
  itemIcon?: Panel.ItemIcon | null
  size?: number // 默认70
  forceBackground?: string // 强制背景色
  title?: string // 图片没有加载出来时，文字回退取标题首字
}

const props = withDefaults(defineProps<Prop>(), { size: 70 })
const themeVars = useThemeVars()
const defaultBackground = 'var(--yin-component-app-icon-surface, #2a2a2a6b)'
const defaultStyle = ref({
  width: `${props.size}px`,
  height: `${props.size}px`,
})
const imageSrc = computed(() => props.itemIcon?.src?.trim() || '')
const iconBackground = computed(() => props.forceBackground ?? normalizeIconBackground(props.itemIcon?.backgroundColor))
// A user-chosen tile colour keeps its own readable glyph, so a pale avatar does
// not take the scheme's light text on a light background. A translucent colour is
// flattened over the current provider's body colour first.
const iconTextColor = computed(() => readableTextColor(iconBackground.value, themeVars.value.bodyColor))
const imageLoaded = ref(false)
// 文字是垫底层：图片真正加载成功前先显示文字，失败则一直显示文字。
const fallbackText = computed(() =>
  props.itemIcon?.text?.trim() || (props.title || '').trim().slice(0, 1) || '?',
)

watch(imageSrc, () => {
  imageLoaded.value = false
})

const handleImageLoad = () => {
  imageLoaded.value = true
}

const handleImageError = (event: Event) => {
  imageLoaded.value = false
  ;(event.target as HTMLImageElement).style.display = 'none'
}
</script>

<template>
  <div class="item-icon" :style="defaultStyle">
    <slot>
      <template v-if="itemIcon">
        <template v-if="itemIcon?.itemType === 1">
          <NAvatar :size="props.size" :style="{ backgroundColor: iconBackground || defaultBackground }">
            <span :style="iconTextColor ? { color: iconTextColor } : undefined">{{ itemIcon.text }}</span>
          </NAvatar>
        </template>

        <template v-else-if="itemIcon?.itemType === 2">
          <div :style="{ position: 'relative', backgroundColor: iconBackground || defaultBackground, ...defaultStyle }" class="flex justify-center items-center">
            <span v-if="!imageLoaded" class="leading-none" :style="iconTextColor ? { color: iconTextColor } : undefined">{{ fallbackText }}</span>
            <img
              v-if="imageSrc"
              :src="imageSrc"
              :style="{ position: 'absolute', inset: '0', width: '100%', height: '100%', objectFit: 'contain', opacity: imageLoaded ? '1' : '0' }"
              alt=""
              loading="lazy"
              decoding="async"
              @load="handleImageLoad"
              @error="handleImageError"
            >
          </div>
        </template>

        <template v-else-if="itemIcon?.itemType === 3">
          <NAvatar :size="props.size" :style="{ backgroundColor: iconBackground || defaultBackground }">
            <span :style="iconTextColor ? { color: iconTextColor } : undefined">
              <SvgIconOnline :style="{ fontSize: 'var(--yin-component-iconography-size)' }" :icon="itemIcon.text" />
            </span>
          </NAvatar>
        </template>
      </template>

      <template v-else>
        <NAvatar :size="props.size" />
      </template>
    </slot>
  </div>
</template>
