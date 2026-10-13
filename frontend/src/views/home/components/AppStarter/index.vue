<script setup lang="ts">
import { computed, onMounted, onUnmounted, provide, ref, watch } from 'vue'
import { NLayout, NLayoutContent, NLayoutSider, NSpace } from 'naive-ui'
import { useAuthStore } from '../../../../store'
import AppLoader from '../../../../components/common/AppLoader/index.vue'
import RoundCardModal from '../../../../components/common/RoundCardModal/index.vue'
import SvgIcon from '../../../../components/common/SvgIcon/index.vue'
import { openSettingsAppKey } from '../../../../components/apps/navigation'
import { t } from '../../../../locales'

interface App {
  name: string
  componentName: string
  icon: string
  auth?: number
}

interface AppGroup {
  /** Group label; the fallback group (About) carries no label on purpose. */
  title: string
  items: App[]
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

const componentName = ref('Account')
const collapsed = ref(false)
const screenWidth = ref(0)
const isSmallScreen = ref(false)
const defaultTitle = t('appLauncher.title')
const title = ref('')
const height = ref('500px')

const authStore = useAuthStore()
const isAdmin = computed(() => Number(authStore.userInfo?.role) === 1)

/**
 * The apps are grouped by the scope they change — the user's own account, the
 * selected space, or the whole instance — so a page's reach is readable before
 * it is opened. The admin group disappears entirely for a non-admin.
 */
const menuGroups = computed<AppGroup[]>(() => [
  {
    title: t('appLauncher.groups.personal'),
    items: [
      {
        name: t('apps.account.appName'),
        componentName: 'Account',
        icon: 'material-symbols-person-edit-outline-rounded',
      },
      {
        name: t('apps.appearance.appName'),
        componentName: 'Appearance',
        icon: 'ion-color-palette-outline',
      },
      {
        name: t('apps.layout.appName'),
        componentName: 'Layout',
        icon: 'lucide-layout-dashboard',
      },
      {
        name: t('themeWallpaper.center'),
        componentName: 'WallpaperCenter',
        icon: 'lucide-wallpaper',
      },
    ],
  },
  {
    title: t('appLauncher.groups.space'),
    items: [
      {
        name: t('spaceManage.title'),
        componentName: 'SpaceManage',
        icon: 'ic-baseline-add-business',
      },
    ],
  },
  {
    title: t('appLauncher.groups.admin'),
    items: [
      {
        name: t('apps.instance.appName'),
        componentName: 'Instance',
        icon: 'lucide-server',
        auth: 1,
      },
      {
        name: t('adminSettingUsers.appName'),
        componentName: 'Users',
        icon: 'lucide-users',
        auth: 1,
      },
    ].filter(item => isAdmin.value),
  },
  {
    title: '',
    items: [
      {
        name: t('apps.about.appName'),
        componentName: 'About',
        icon: 'lucide-info',
      },
    ],
  },
])

/** Switch the modal to another app: the appearance page points at the wallpaper
 *  centre instead of embedding a second picker. */
function openSettingsApp(name: string) {
  componentName.value = name
  if (isSmallScreen.value)
    collapsed.value = true
}
provide(openSettingsAppKey, openSettingsApp)

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
              <div class="w-full h-full app-starter-rail">
                <div
                  class="p-[5px] app-starter-menu rounded-xl overflow-auto"
                  :style="{
                    width: isSmallScreen ? '100%' : '220px',
                    minWidth: '200px',
                    height,
                  }"
                >
                  <template v-for="(group, groupIndex) in menuGroups" :key="groupIndex">
                    <div
                      v-if="group.title"
                      class="px-[6px] pt-[8px] pb-[2px] text-xs font-bold yin-text-muted"
                    >
                      {{ group.title }}
                    </div>
                    <div
                      v-for="item in group.items"
                      :key="item.componentName"
                      :style="{ color: componentName === item.componentName ? 'var(--yin-primary)' : '' }"
                      @click="handleClickApp(item)"
                    >
                      <div
                        class="app-starter-menu-item p-[10px] rounded-lg mb-[5px] font-bold cursor-pointer flex items-center"
                        :data-testid="'settings-app-' + item.componentName"
                      >
                        <div class="flex items-center justify-center">
                          <div class="text-lg">
                            <SvgIcon :icon="item.icon" />
                          </div>
                          <span class="ml-2">{{ item.name }}</span>
                        </div>
                      </div>
                    </div>
                  </template>
                </div>
              </div>
            </NLayoutSider>
            <NLayoutContent :content-style="{ height }">
              <div class="app-starter-pane rounded-2xl h-full overflow-auto transition-all duration-500 min-w-[300px] h-full" :class="(isSmallScreen && !collapsed) ? 'opacity-0' : 'opacity-100'">
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
/* The settings modal is built from the theme's two surface tokens, so its depth
   is deliberate rather than incidental: the rail and the workspace an app renders
   in sit recessed on the canvas, and everything that should read as raised — the
   menu items, the cards, the tables — sits on the surface. The rail and the menu
   share the canvas on purpose, so no strip of a second colour shows between them.
   The hard-coded greys this replaces (`#2c2c32`, Tailwind zinc) sat outside the
   theme tokens, so they neither followed the active theme nor agreed with each
   other. */
.app-starter-modal-content .n-layout,
.app-starter-modal-content .n-layout-sider,
.app-starter-modal-content .n-layout-content,
.app-starter-modal-content .n-layout-scroll-container,
.app-starter-rail,
.app-starter-menu,
.app-starter-pane {
    background-color: var(--yin-canvas);
}

/* The app pages render on that recessed workspace, so the cards they draw are
   what lifts off it. Scoped to the modal: `theme-page` keeps its own definition
   everywhere else, including for themes. */
.app-starter-modal-content .theme-page {
    background-color: transparent;
}

/* The line separates the two recessed areas, which now share a depth. */
.app-starter-modal-content .n-layout-sider {
    border-inline-end: 1px solid var(--yin-border);
}

/* Raised on the recessed rail. The active item is marked by its text colour,
   which the template binds to `--yin-primary`. */
.app-starter-menu-item {
    background-color: var(--yin-surface);
    border: 1px solid transparent;
    transition: background-color var(--yin-component-state-hover-duration) var(--yin-component-state-easing), border-color var(--yin-component-state-hover-duration) var(--yin-component-state-easing);
}

.app-starter-menu-item:hover,
.app-starter-menu-item:focus-visible {
    background-color: color-mix(in srgb, var(--yin-primary) 10%, var(--yin-surface));
    border-color: color-mix(in srgb, var(--yin-primary) 30%, var(--yin-surface));
}

/* The cards drop the drop shadow: the step from the page's canvas to the card's
   surface already separates them, and the users page never had a shadow, so the
   others looked inconsistent. */
.app-starter-modal-content .n-card {
    box-shadow: none;
}
</style>
