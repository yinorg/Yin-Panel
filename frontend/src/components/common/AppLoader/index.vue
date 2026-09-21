<script setup lang="ts">
import { defineAsyncComponent, onMounted, shallowRef, watch, type Component } from 'vue'
import { NSpin } from 'naive-ui'

const props = defineProps<{
  componentName: string | null
}>()
const emit = defineEmits<{
  (e: 'spaces-changed'): void
}>()
const loading = shallowRef(false)
const dynamicComponent = shallowRef<Component | null>(null)

const componentLoaders: Record<string, () => Promise<Component>> = {
  UserInfo: () => import('../../apps/UserInfo/index.vue'),
  Style: () => import('../../apps/Style/index.vue'),
  SpaceManage: () => import('../../apps/SpaceManage/index.vue'),
  UploadFileManager: () => import('../../apps/UploadFileManager/index.vue'),
  About: () => import('../../apps/About/index.vue'),
  Users: () => import('../../apps/Users/index.vue'),
}

function updateComponent() {
  loading.value = true
  const loader = componentLoaders[props.componentName || '']
  if (!loader) {
    dynamicComponent.value = null
    loading.value = false
    return
  }
  dynamicComponent.value = defineAsyncComponent<Component>(() =>
    loader()
      .finally(() => {
        loading.value = false
      }).catch(() => {
      // 组件不存在
        dynamicComponent.value = null
        return { render: () => null }
      }),
  )
}

watch(() => props.componentName, () => {
  updateComponent()
})

onMounted(() => {
  updateComponent()
})
</script>

<template>
  <div class="h-full">
    <NSpin :show="loading" style="height: 100%;" content-style="height: 100%;" :delay="500" description="loading...">
      <component :is="dynamicComponent" v-if="dynamicComponent" @spaces-changed="emit('spaces-changed')" />
      <!-- <component :is="getComponent(componentName || '')" v-if="dynamicComponent" /> -->
      <div
        v-else-if="!dynamicComponent"
      >
        Component not found!
      </div>
    </NSpin>
  </div>
</template>
