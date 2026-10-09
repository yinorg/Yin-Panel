<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { NLayout, NLayoutContent, NLayoutSider, NSpace } from 'naive-ui'
import { useAuthStore } from '../../../../store'
import AppLoader from '../../../../components/common/AppLoader/index.vue'
import RoundCardModal from '../../../../components/common/RoundCardModal/index.vue'
import SvgIcon from '../../../../components/common/SvgIcon/index.vue'
import { t } from '../../../../locales'

interface App {
  name: string
  componentName: string
  icon: string
  auth?: number
}
const props = defineProps<{
  visible: boolean
  /** Which app to open on when the modal becomes visible; set by the Core when a
   *  theme does not contribute a surface and the Core page is the fallback. */
  initialComponent?: string
}>()

const emit = defineEmits<{
  (e: 'update:visible', visible: boolean): void
  (e: 'spaces-changed'): void
}>()

const componentName = ref('UserInfo')
const collapsed = ref(false)
const screenWidth = ref(0)
const isSmallScreen = ref(false)
const defaultTitle = t('appLauncher.title')
const title = ref('')
const height = ref('500px')

const apps = ref<App[]>([
  {
    name: t('apps.userInfo.appName'),
    componentName: 'UserInfo',
    icon: 'material-symbols-person-edit-outline-rounded',
  },
  {
    name: t('apps.baseSettings.appName'),
    componentName: 'Style',
    icon: 'ion-color-palette-outline',
  },
  {
    name: t('spaceManage.title'),
    componentName: 'SpaceManage',
    icon: 'ic-baseline-add-business',
  },
])

const authStore = useAuthStore()

// Open on the app the Core asked for (a surface fallback); otherwise keep the
// last-selected app so reopening the modal is stable.
watch(() => props.visible, (visible) => {
  if (visible && props.initialComponent)
    componentName.value = props.initialComponent
})

const show = computed({
  get: () => props.visible,
  set: (visible: boolean) => {
    emit('update:visible', visible)
  },
})

function handleClickApp(item: App) {
  componentName.value = item.componentName
  if (isSmallScreen.value)
    collapsed.value = true
}

function toggleCollapsed() {
  collapsed.value = !collapsed.value
}

function getScreenWidth() {
  return window.innerWidth
}

function handleResize() {
  screenWidth.value = getScreenWidth()
  if (screenWidth.value < 640) {
    collapsed.value = true
    isSmallScreen.value = true
  }
  else {
    collapsed.value = false
    isSmallScreen.value = false
  }
}

onMounted(() => {
  const adminApp: App = {
    name: t('adminSettingUsers.appName'),
    componentName: 'Users',
    icon: 'lucide-users',
    auth: 1,
  }
  const aboutApp: App = {
    name: t('apps.about.appName'),
    componentName: 'About',
    icon: 'lucide-info',
  }
  // 初始化
  const addAdminApp = () => {
    if (Number(authStore.userInfo?.role) !== 1)
      return
    if (!apps.value.some(item => item.componentName === 'Users'))
      apps.value.push(adminApp)
  }
  addAdminApp()
  apps.value.push(aboutApp)
  watch(() => authStore.userInfo?.role, addAdminApp)

  window.addEventListener('resize', handleResize)
  handleResize()
})

onUnmounted(() => {
  window.removeEventListener('resize', handleResize)
})
</script>

<template>
  <div>
    <RoundCardModal
      v-model:show="show"
      style="max-width: 900px;"
      size="small"
    >
      <template #header>
        <!--
          The header toggle is the first focusable element inside the modal, and it is
          rendered synchronously with the dialog. vueuc's FocusTrap (used internally by
          NModal) resolves its initial focus once, on mount, by looking for a focusable
          descendant; the settings pages it hosts arrive later as an async chunk, so at
          that moment there is nothing to focus and the trap falls back to parking focus
          on its own aria-hidden 0x0 sentinel, which the browser then reports as
          "Blocked aria-hidden on an element because its descendant retained focus".
          Being focusable here keeps the trap on its intended path, and it also makes the
          collapse toggle reachable by keyboard, which it previously was not.
        -->
        <div
          class="flex items-center select-none"
          role="button"
          tabindex="0"
          data-testid="app-starter-header-toggle"
          @click="toggleCollapsed"
          @keydown.enter.prevent="toggleCollapsed"
          @keydown.space.prevent="toggleCollapsed"
        >
          <div class="text-3xl cursor-pointer" style="color:var(--n-color-target)">
            <SvgIcon class=" transition-all duration-500" :icon="collapsed ? 'tabler-layout-sidebar-right-collapse-filled' : 'tabler-layout-sidebar-left-collapse-filled'" />
          </div>
          <div class="ml-1">
            {{ title === '' ? defaultTitle : title }}
          </div>
        </div>
      </template>
      <div class="w-full h-full app-starter-modal-content">
        <NSpace vertical size="large" style="height: 100%;width: 100%;">
          <NLayout has-sider style="border-radius:0.75rem;">
            <NLayoutSider
              v-model:collapsed="collapsed"
              collapse-mode="width"
              :collapsed-width="0"
              :width="isSmallScreen ? '100%' : 240"
              style="height: 100%;"
              content-style="overflow: hidden"
            >
              <div class="w-full h-full dark:bg-[#2c2c32]">
                <div
                  class="p-[5px] bg-slate-200 dark:bg-zinc-900 rounded-xl overflow-auto"
                  :style="{
                    width: isSmallScreen ? '100%' : '220px',
                    minWidth: '200px',
                    height,
                  }"
                >
                  <div
                    v-for=" (item, index) in apps"
                    :key="index"
                    :style="{ color: componentName === item.componentName ? 'var(--n-color-target)' : '' }"
                    @click="handleClickApp(item)"
                  >
                    <div
                      class="bg-white dark:bg-zinc-800 p-[10px] rounded-lg mb-[5px] font-bold cursor-pointer flex items-center hover:bg-slate-50 focus:bg-slate-50"
                    >
                      <div class="flex items-center justify-center">
                        <div class="text-lg">
                          <SvgIcon :icon="item.icon" />
                        </div>
                        <span class="ml-2">{{ item.name }}</span>
                      </div>
                    <!-- 更多按钮 -->
                    <!-- <div class="ml-auto">
                      <SvgIcon icon="mingcute-more-1-fill" />
                    </div> -->
                    </div>
                  </div>
                </div>
              </div>
            </NLayoutSider>
            <NLayoutContent :content-style="{ height }">
              <div class="rounded-2xl h-full overflow-auto transition-all duration-500 min-w-[300px] h-full" :class="(isSmallScreen && !collapsed) ? 'opacity-0' : 'opacity-100'">
                <AppLoader :component-name="componentName" class="h-full" @spaces-changed="emit('spaces-changed')" />
              </div>
            </NLayoutContent>
          </NLayout>
        </NSpace>
      </div>
    </RoundCardModal>
  </div>
</template>

<style scoped>
.text-shadow {
  text-shadow: 0px 0px 5px gray;
}
</style>

<style>
.dark .app-starter-modal-content .n-layout{
    background-color: #2c2c32;
}
</style>
