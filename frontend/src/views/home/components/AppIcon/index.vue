<script setup lang="ts">
import { computed, ref } from 'vue'
import { NEllipsis } from 'naive-ui'
import ItemIcon from '../../../../components/common/ItemIcon/index.vue'
import { PanelPanelConfigStyleEnum } from '@/enums'

interface Prop {
  itemInfo?: Panel.ItemInfo
  size?: number // 默认70
  forceBackground?: string // 强制背景色
  iconTextColor?: string
  iconTextInfoHideDescription: boolean
  iconTextIconHideTitle: boolean
  style: PanelPanelConfigStyleEnum
  directory?: boolean
}

const props = withDefaults(defineProps<Prop>(), {
  size: 70,
  directory: false,
})

const directoryIconSize = 30
const defaultBackground = '#2a2a2a6b'

const calculateLuminance = (color: string) => {
  const hex = color.replace(/^#/, '')
  const r = parseInt(hex.substring(0, 2), 16)
  const g = parseInt(hex.substring(2, 4), 16)
  const b = parseInt(hex.substring(4, 6), 16)
  return (0.299 * r + 0.587 * g + 0.114 * b) / 255
}

const textColor = computed(() => {
  const luminance = calculateLuminance(props.itemInfo?.icon?.backgroundColor || defaultBackground)
  return luminance > 0.5 ? 'black' : 'white'
})

// Card tilt effect variables
const isHovering = ref(false)
const cardTransform = ref({ x: 0, y: 0 })

// Handle mouse events
const handleMouseEnter = () => {
  isHovering.value = true
}

const handleMouseLeave = () => {
  isHovering.value = false
  // Reset transform on mouse leave
  cardTransform.value = { x: 0, y: 0 }
}

const handleMouseMove = (e: MouseEvent, element: EventTarget | null) => {
  if (!(element instanceof HTMLElement) || !isHovering.value) return
  
  const rect = element.getBoundingClientRect()
  const centerX = rect.left + rect.width / 2
  const centerY = rect.top + rect.height / 2
  
  // Calculate distance from center (normalized to -1 to 1)
  const x = (e.clientX - centerX) / (rect.width / 2)
  const y = (e.clientY - centerY) / (rect.height / 2)
  
  const tiltDegrees = Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--yin-component-state-tilt-degrees')) || 0
  cardTransform.value = {
    x: y * -tiltDegrees,
    y: x * tiltDegrees,
  }
  
  // Update glow position
  if (element) {
    // Calculate relative position for the glow effect (0-100%)
    const relativeX = ((e.clientX - rect.left) / rect.width) * 100
    const relativeY = ((e.clientY - rect.top) / rect.height) * 100
    element.style.setProperty('--x', `${relativeX}%`)
    element.style.setProperty('--y', `${relativeY}%`)
  }
}

// Computed styles for card transform
const cardStyle = computed(() => {
  if (!isHovering.value) {
    return {
      transform: 'perspective(1000px) rotateX(0deg) rotateY(0deg)',
    }
  }
  
  return {
    transform: `perspective(1000px) rotateX(${cardTransform.value.x}deg) rotateY(${cardTransform.value.y}deg)`,
  }
})
</script>

<template>
  <div
    class="app-icon w-full"
    :class="{ 'app-icon--directory': directory }"
    :style="directory ? { '--app-icon-directory-size': `${directoryIconSize}px` } : undefined"
    data-testid="home-item"
    :data-item-title="itemInfo?.title || ''"
  >
    <!-- 详情图标 -->
    <div
      v-if="style === PanelPanelConfigStyleEnum.info"
      class="app-icon-info w-full flex card-container"
      :style="[
        { background: itemInfo?.icon?.backgroundColor || defaultBackground },
        cardStyle
      ]"
      @mouseenter="handleMouseEnter"
      @mouseleave="handleMouseLeave"
      @mousemove="(e) => handleMouseMove(e, e.currentTarget)"
    >
      <!-- 图标 -->
      <div class="app-icon-info-icon">
        <div class="app-icon-info-icon-inner flex items-center justify-center">
          <ItemIcon :item-icon="itemInfo?.icon" force-background="transparent" :size="50" class="app-icon-glyph" />
        </div>
      </div>

      <!-- 文字 -->
      <!-- 如果为纯白色，将自动根据背景的明暗计算字体的黑白色 -->
      <div class="text-white flex items-center" :style="{ color: (iconTextColor === '#ffffff') ? textColor : iconTextColor, maxWidth: 'calc(100% - 80px)' }">
        <div class="app-icon-info-text-box w-full">
          <div class="app-icon-info-text-box-title w-full">
            <NEllipsis>
              {{ itemInfo?.title }}
            </NEllipsis>
          </div>
          <div v-if="!iconTextInfoHideDescription" class="app-icon-info-text-box-description">
            <NEllipsis :line-clamp="2" class="app-icon-info-text-box-description">
              {{ itemInfo?.description }}
            </NEllipsis>
          </div>
        </div>
      </div>
      
      <!-- Hover glow effect -->
      <div class="card-glow"></div>
    </div>

    <!-- 极简(小)图标（APP） -->
    <div v-if="style === PanelPanelConfigStyleEnum.icon" class="app-icon-small" :class="{ 'app-icon-small--directory': directory }">
      <div
        class="app-icon-small-icon overflow-hidden sunpanel mx-auto card-container"
        :title="itemInfo?.description"
        :style="cardStyle"
        @mouseenter="handleMouseEnter"
        @mouseleave="handleMouseLeave"
        @mousemove="(e) => handleMouseMove(e, e.currentTarget)"
      >
        <ItemIcon :item-icon="itemInfo?.icon" :size="directory ? directoryIconSize : size" />
        <!-- Hover glow effect -->
        <div class="card-glow"></div>
      </div>
      <div
        v-if="!iconTextIconHideTitle"
        class="app-icon-small-title app-icon-text-shadow cursor-pointer"
        :style="{ color: iconTextColor }"
      >
        <span>{{ itemInfo?.title }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.card-container {
  position: relative;
  transform-style: preserve-3d;
  will-change: transform;
  overflow: hidden;
  border: var(--yin-component-card-border-width) var(--yin-component-card-border-style) transparent;
  backface-visibility: hidden;
}

.card-glow {
  position: absolute;
  width: 100%;
  height: 100%;
  top: 0;
  left: 0;
  background: radial-gradient(circle at var(--x, 50%) var(--y, 50%), color-mix(in srgb, var(--yin-primary) calc(var(--yin-effect-glow-opacity) * 100%), transparent) 0%, transparent 60%);
  opacity: 0;
  pointer-events: none;
  transition: opacity var(--yin-component-state-hover-duration) var(--yin-component-state-easing);
}

.card-container:hover .card-glow {
  opacity: 1;
}

.app-icon-info:hover, .app-icon-small-icon:hover {
  box-shadow: var(--yin-component-app-icon-shadow);
  transform: translateY(calc(var(--yin-component-surface-glow) * -0.15));
  border-color: var(--yin-border);
}

.app-icon-info, .app-icon-small-icon {
  border-radius: var(--yin-component-app-icon-radius);
  transition: transform var(--yin-component-state-hover-duration) var(--yin-component-state-easing), box-shadow var(--yin-component-state-hover-duration) var(--yin-component-state-easing), border-color var(--yin-component-state-hover-duration) var(--yin-component-state-easing);
}
.app-icon-info-icon { width: var(--yin-component-app-icon-size); height: var(--yin-component-app-icon-size); flex: 0 0 var(--yin-component-app-icon-size); }
.app-icon-info-icon-inner { width: 100%; height: 100%; }
.app-icon-info:hover, .app-icon-small-icon:hover { scale: var(--yin-component-state-hover-scale); }
.app-icon-small-icon { width: var(--yin-component-app-icon-size); height: var(--yin-component-app-icon-size); }
.app-icon-small-title { margin-top: var(--yin-spaceXs); text-align: center; font-family: var(--yin-fontBody); font-size: var(--yin-fontSmallSize); font-weight: var(--yin-fontHeadingWeight); }
.app-icon-info-text-box-title { font-weight: var(--yin-fontHeadingWeight); }
.app-icon-info-text-box-description { font-size: var(--yin-fontSmallSize); }
.app-icon-glyph :deep(.item-icon) { overflow: hidden; border-radius: var(--yin-component-iconography-container-radius); }
.app-icon-glyph :deep(svg) { stroke-width: var(--yin-component-iconography-stroke-width); }
.app-icon--directory { width: auto; max-width: 100%; }
.app-icon-small--directory { display: flex; align-items: center; gap: var(--yin-component-app-icon-gap); }
.app-icon-small--directory .app-icon-small-icon {
  flex: 0 0 var(--app-icon-directory-size);
  width: var(--app-icon-directory-size);
  height: var(--app-icon-directory-size);
  margin: 0;
  border-radius: var(--yin-component-iconography-container-radius);
}
.app-icon-small--directory .app-icon-small-title {
  min-width: 0;
  max-width: 36ch;
  margin: 0;
  font-size: var(--yin-fontBodySize);
  color: var(--yin-text) !important;
  text-align: left;
  text-shadow: none;
  white-space: normal;
  overflow-wrap: anywhere;
}
:global(:root[data-yin-density='compact']) .app-icon-small { margin-bottom: 0; }
:global(:root[data-yin-surface='glass']) .app-icon-small-icon,
:global(:root[data-yin-surface='frosted']) .app-icon-small-icon { backdrop-filter: blur(var(--yin-component-surface-blur)); }
:global(:root[data-yin-surface='gradient']) .app-icon-small-icon { box-shadow: 0 0 var(--yin-component-surface-glow) var(--yin-primary); }
</style>
