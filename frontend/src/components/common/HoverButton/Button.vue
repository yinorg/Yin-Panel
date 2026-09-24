<script setup lang='ts'>
interface Emit {
  (e: 'click'): void
}

const emit = defineEmits<Emit>()

function handleClick() {
  emit('click')
}
</script>

<template>
  <button
    class="theme-hover-button flex items-center justify-center w-10 h-10"
    @click="handleClick"
  >
    <slot />
  </button>
</template>

<style scoped>
.theme-hover-button {
  border: var(--yin-component-button-border-width) solid transparent;
  border-radius: var(--yin-component-iconography-container-radius);
  color: var(--yin-text);
  transition: background-color var(--yin-component-state-hover-duration) var(--yin-component-state-easing), border-color var(--yin-component-state-hover-duration) var(--yin-component-state-easing), transform var(--yin-component-state-press-duration) var(--yin-component-state-easing);
}
.theme-hover-button:hover { background: color-mix(in srgb, var(--yin-primary) 12%, transparent); border-color: var(--yin-border); }
.theme-hover-button:active { transform: scale(var(--yin-component-state-press-scale)); }
</style>
