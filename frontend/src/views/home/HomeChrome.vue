<script setup lang="ts">
import { NButton, NCard, NCheckbox, NDropdown, NInput, NModal, NRadio, NRadioGroup, NSkeleton, NSpin, NSpace } from 'naive-ui'
import { defineAsyncComponent } from 'vue'
import CommandCenter from './components/CommandCenter/index.vue'
import WallpaperLayer from './components/WallpaperLayer.vue'
import HomeFallback from './components/HomeFallback.vue'
import ThemeHost from '@/theme/runtime/ThemeHost.vue'
import type { useHomeShell } from '@/core/home/useHomeShell'
// Shell chrome styles live beside the host rather than inline. See that file's
// header for the scoping note.
import './home-shell.css'

type Shell = ReturnType<typeof useHomeShell>

/**
 * The Core-owned chrome around the theme frame: everything the home page renders
 * that a theme cannot render for itself (space switcher, connectivity status,
 * dialogs, the command centre entry, the consent prompt and the C3 fallback).
 *
 * It is a pure view over `useHomeShell`'s view model. The four groups are separate
 * props rather than one object so each section's dependencies are legible; nothing
 * here reaches into a store or a composable directly.
 */
defineProps<{
  theme: Shell['theme']
  home: Shell['home']
  dialogs: Shell['dialogs']
  commands: Shell['commands']
}>()

const SystemMonitor = defineAsyncComponent(() => import('@/components/deskModule/SystemMonitor/index.vue'))
const AppStarter = defineAsyncComponent(() => import('./components/AppStarter/index.vue'))
const EditItem = defineAsyncComponent(() => import('./components/EditItem/index.vue'))
</script>

<template>
  <div class="w-full h-full sun-main" :class="{ 'side-switching': home.sideSwitching, 'theme-defaults': theme.useThemeColors, 'wallpaper-active': theme.wallpaperActive, 'theme-runtime-yin': theme.themeRuntimeActive && theme.themeRuntimePackage?.manifest.id === 'org.yin.default' }" :data-panel-side="home.activeSpace?.side || 'yin'">
    <CommandCenter
      :visible="commands.commandCenterVisible"
      :query="commands.commandCenterQuery"
      :items="commands.commandCenterItems"
      :commands="commands.filteredCommandDefinitions"
      :selected-index="commands.commandCenterSelectedIndex"
      :search-engine="commands.commandCenterSearchEngine"
      @update:query="commands.updateCommandCenterQuery"
      @move="commands.moveCommandSelection"
      @select="commands.selectCommandItem"
      @submit-search="commands.submitCommandCenterSearch"
      @execute-item="commands.executeCommandItem"
      @execute-command="commands.executeCommand"
      @close="commands.closeCommandCenter"
    />
    <div v-if="home.sideSwitching" class="taiji-transition" aria-hidden="true">
      <div class="taiji-aura">
        <div class="taiji-bagua">
          <span v-for="index in 8" :key="index" />
        </div>
        <div class="taiji-symbol" :class="home.activeSpace?.side === 'yang' ? 'taiji-yang' : 'taiji-yin'">
          <span class="taiji-dot taiji-dot-dark" />
          <span class="taiji-dot taiji-dot-light" />
        </div>
      </div>
    </div>
    <NModal :show="!!home.publicCode && !home.publicAccessReady && !home.publicAccessChecking" :mask-closable="false" :closable="false">
      <NCard :title="$t('spaceManage.accessVerification')" style="width: min(92vw, 380px)">
        <NSpace vertical>
          <span>{{ $t('spaceManage.enterAccessCode') }}</span>
          <NInput :value="home.publicAccessCode" type="password" show-password-on="click" maxlength="12" :placeholder="$t('spaceManage.accessCodeShortPlaceholder')" @keyup.enter="home.unlockPublicAccess" @update:value="home.setPublicAccessCode" />
          <NButton type="primary" block @click="home.unlockPublicAccess">{{ $t('spaceManage.confirmAccess') }}</NButton>
        </NSpace>
      </NCard>
    </NModal>
    <div v-if="home.showSpaceBar" class="space-status-bar">
      <NDropdown
        trigger="hover"
        :options="home.spaceSelectorOptions"
        :theme-overrides="{
          color: 'rgba(18, 22, 28, 0.72)',
          optionTextColor: 'rgba(255, 255, 255, 0.92)',
          optionTextColorHover: '#fff',
          optionColorHover: 'rgba(255, 255, 255, 0.14)',
          optionColorActive: 'rgba(125, 211, 252, 0.18)',
          dividerColor: 'rgba(255, 255, 255, 0.18)',
          borderRadius: '10px',
        }"
        @select="home.selectSpace"
      >
        <NButton quaternary class="space-status-button">
          <span class="space-status-dot" />
          {{ home.activeSpaceLabel }}
          <span class="ml-2 opacity-60">⌄</span>
        </NButton>
      </NDropdown>
    </div>
    <!-- Public links have no space selector; nothing should occupy that spot. -->
    <div v-if="home.homeReady && !home.publicCode" class="offline-status" :class="{ 'offline-status--theme-hidden': theme.themeRuntimeActive && home.isOnline }" data-testid="offline-status">
      <span v-if="home.pwaReady" data-testid="pwa-ready">{{ $t('panelHome.pwaReady') }}</span>
      <template v-if="!home.isOnline && home.hasValidCachedHome">
        <span data-testid="offline-readonly">{{ $t('panelHome.offlineReadonly') }}</span>
        <span v-if="home.cacheUpdatedAt" data-testid="offline-cache-updated">{{ $t('panelHome.cacheUpdatedAt', { time: new Date(home.cacheUpdatedAt).toLocaleString() }) }}</span>
      </template>
    </div>
    <div v-if="theme.themeSafeMode && home.homeReady" class="theme-safe-mode-banner" role="status" data-testid="theme-safe-mode-banner">
      <span>{{ $t('themeRecovery.active') }}</span>
      <RouterLink to="/__yin/theme-recovery">{{ $t('themeRecovery.exit') }}</RouterLink>
    </div>
    <WallpaperLayer v-if="home.homeReady" />
    <ThemeHost
      v-if="theme.themeRuntimeActive && theme.themeRuntimePackage"
      :key="`${theme.themeRuntimePackage.revision}:${theme.themeExecutionMode}:${theme.themeRuntimeGrant.permissions.join(',')}`"
      class="theme-home-host"
      :ref="theme.setThemeHostElement"
      :theme="theme.themeRuntimePackage"
      :snapshot="theme.themeRuntimeSnapshot"
      :environment="theme.themeRuntimeEnvironment"
      :permissions="theme.themeRuntimePermissions"
      :execution-mode="theme.themeExecutionMode"
      :slots="theme.themeRuntimeSlots"
      :title="theme.themeRuntimePackage.manifest.name"
      :execute="theme.executeThemeRequest"
      @failed="theme.handleThemeRuntimeFailure"
    >
      <div
        v-if="theme.monitorVisible"
        :ref="theme.setMonitorLayerElement"
        class="theme-runtime-monitor-layer"
        :class="{ 'theme-runtime-monitor-layer--info': theme.monitorIsInfo, 'theme-runtime-monitor-layer--yin': theme.themeRuntimePackage?.manifest.id === 'org.yin.default' }"
        data-testid="theme-runtime-monitor"
        :data-panel-side="home.activeSpace?.side || 'yin'"
        :style="{ '--yin-theme-content-top': `${theme.contentTopVh}vh`, '--yin-theme-monitor-top': theme.themeMonitorTop }"
      >
        <SystemMonitor
          :snapshot-controller="theme.monitorSnapshotController"
          :show-title="theme.monitorShowTitle"
          :icon-text-color="theme.panelIconTextColor"
        />
      </div>
    </ThemeHost>
    <!-- Boundary C3: the theme cannot render the home, so show the minimum viable
         list rather than a blank page. Mutually exclusive with the theme frame so
         exactly one full view is ever mounted. -->
    <HomeFallback
      v-if="theme.homeFallbackVisible"
      :spaces="home.spaces"
      :active-space-id="home.activeSpace?.id"
      :groups="home.items"
      :reason="theme.homeFallbackReason"
      :brand="theme.brandText"
      @select-space="home.selectSpace"
      @open-item="home.openFallbackItem"
      @refresh="home.refreshCurrentSpace"
    />
    <div v-if="home.offlineUnavailable" class="offline-unavailable" data-testid="offline-unavailable">
      <NCard :title="$t('panelHome.offlineUnavailable')" size="small">
        <NSpace vertical>
          <span>{{ $t('panelHome.offlineUnavailableDetail') }}</span>
          <NButton type="primary" data-testid="offline-retry-button" @click="home.retryWhenOnline">{{ $t('panelHome.retryWhenOnline') }}</NButton>
        </NSpace>
      </NCard>
    </div>
    <!-- Boundary C3: the theme is mounted but still needs the user's permission to
         use its capabilities. This is the only Core prompt that has to appear *over*
         a live theme, so it sits in an absolute layer pinned below the theme's own
         top chrome (space bar / action bar) rather than in the content flow it used
         to occupy. -->
    <div v-if="dialogs.themeRuntimeNeedsConsent" class="theme-runtime-notice-layer">
      <div class="theme-runtime-notice" role="status" data-testid="theme-runtime-consent">
        <span>{{ dialogs.themeRuntimeFailureMessage || $t('themePackage.runtimePrompt', { name: theme.themeRuntimePackage?.manifest.name, permissions: dialogs.themeRequiredPermissions.join(', ') || $t('themePackage.noRuntimePermissions') }) }}</span>
        <NButton size="small" type="primary" @click="dialogs.openThemeRuntimeConsent">
          {{ $t('themeTrustedRuntime.review') }}
        </NButton>
      </div>
    </div>

    <!-- The settings surface stays Core-owned (D5) and is opened by the command
         center's `settings` command, so it must remain mounted now that the
         floating button group that used to host it is gone. -->
    <AppStarter :visible="dialogs.settingModalShow" @spaces-changed="dialogs.handleSpacesChanged" @update:visible="dialogs.setSettingModalShow" />

    <!--
      `@update:visible` was missing here, so the modal emitted its close and nobody
      received it: `props.visible` stayed true and the dialog never went away. Worse,
      its mask kept intercepting pointer events, so the whole page became unclickable.
      `dialogs.setEditItemInfoShow` already existed for this; it simply had no caller.
    -->
    <EditItem :visible="dialogs.editItemInfoShow" :item-info="dialogs.editItemInfoData" :item-group-id="dialogs.currentAddItenIconGroupId" :space-id="home.activeSpace?.id" :mutations="dialogs.homeMutations" @update:visible="dialogs.setEditItemInfoShow" />

    <!-- 弹窗 -->
    <NModal
      :show="dialogs.windowShow" :mask-closable="false" preset="card"
      style="max-width: 1000px;height: 600px;border-radius: 1rem;" :bordered="true" size="small" role="dialog"
      aria-modal="true"
      @update:show="dialogs.setWindowShow"
    >
      <template #header>
        <div class="flex items-center">
          <span class="mr-[20px]">
            {{ dialogs.windowTitle }}
          </span>

          <NSpin v-if="dialogs.windowIframeIsLoad" size="small" />
        </div>
      </template>
      <div class="w-full h-full rounded-2xl overflow-hidden border dark:border-zinc-700">
        <div v-if="dialogs.windowIframeIsLoad" class="flex flex-col p-5">
          <NSkeleton height="50px" width="100%" class="rounded-lg" />
          <NSkeleton height="180px" width="100%" class="mt-[20px] rounded-lg" />
          <NSkeleton height="180px" width="100%" class="mt-[20px] rounded-lg" />
        </div>
        <iframe
          v-show="!dialogs.windowIframeIsLoad" id="windowIframeId" :src="dialogs.windowSrc"
          class="w-full h-full" frameborder="0" @load="dialogs.handWindowIframeIdLoad"
        />
      </div>
    </NModal>
  </div>
  <NModal :show="dialogs.themeRuntimeConsentVisible" preset="card" class="theme-runtime-consent-modal" style="width: min(92vw, 560px)" :title="$t('themeTrustedRuntime.consentTitle')" :mask-closable="false" @update:show="dialogs.setThemeRuntimeConsentVisible">
    <div class="theme-runtime-consent-content" data-testid="theme-runtime-consent-dialog">
      <p class="theme-runtime-consent-package">{{ theme.themeRuntimePackage?.manifest.name }} · {{ theme.themeRuntimePackage?.revision?.slice(0, 12) }}</p>
      <p>{{ $t('themeTrustedRuntime.consentIntro') }}</p>
      <NRadioGroup :value="dialogs.selectedThemeExecutionMode" class="theme-runtime-mode-list" :aria-label="$t('themeTrustedRuntime.modeGroupLabel')" @update:value="dialogs.setSelectedThemeExecutionMode">
        <label class="theme-runtime-mode-option" :class="{ 'theme-runtime-mode-option--selected': dialogs.selectedThemeExecutionMode === 'sandbox' }">
          <NRadio value="sandbox" />
          <span>
              <strong>{{ $t('themeTrustedRuntime.sandboxMode') }}</strong>
              <small>{{ $t('themeTrustedRuntime.sandboxDescription') }}</small>
          </span>
        </label>
        <label class="theme-runtime-mode-option" :class="{ 'theme-runtime-mode-option--selected': dialogs.selectedThemeExecutionMode === 'trusted' }">
          <NRadio value="trusted" :disabled="!dialogs.trustedRuntimeAvailable" />
          <span>
            <strong>{{ $t('themeTrustedRuntime.trustedMode') }}</strong>
            <small>{{ dialogs.trustedRuntimeAvailable ? $t('themeTrustedRuntime.trustedDescription') : $t('themeTrustedRuntime.trustedUnavailable') }}</small>
          </span>
        </label>
      </NRadioGroup>
      <ul class="theme-runtime-permissions" :aria-label="$t('themeTrustedRuntime.permissionsLabel')">
        <li v-for="permission in dialogs.themeRequiredPermissions" :key="permission">{{ permission }}</li>
        <li v-if="!dialogs.themeRequiredPermissions.length">{{ $t('themePackage.noRuntimePermissions') }}</li>
      </ul>
      <section v-if="dialogs.selectedThemeExecutionMode === 'trusted'" class="theme-runtime-risk" role="alert" data-testid="theme-trusted-risk">
        <h3>{{ $t('themeTrustedRuntime.warningTitle') }}</h3>
        <p>{{ $t('themeTrustedRuntime.warning') }}</p>
        <p>{{ $t('themeTrustedRuntime.serverBoundary') }}</p>
        <NCheckbox :checked="dialogs.trustedRuntimeAcknowledged" data-testid="theme-trusted-acknowledgement" @update:checked="dialogs.setTrustedRuntimeAcknowledged">
          {{ $t('themeTrustedRuntime.acknowledge') }}
        </NCheckbox>
      </section>
    </div>
    <template #footer>
      <div class="theme-runtime-consent-actions">
        <NButton quaternary @click="dialogs.setThemeRuntimeConsentVisible(false)">{{ $t('common.cancel') }}</NButton>
        <NButton
          type="primary"
          :loading="dialogs.themeRuntimeGrantSaving"
          :disabled="dialogs.selectedThemeExecutionMode === 'trusted' && (!dialogs.trustedRuntimeAvailable || !dialogs.trustedRuntimeAcknowledged)"
          data-testid="theme-runtime-consent-confirm"
          @click="dialogs.grantThemeRuntimePermissions"
        >
          {{ $t(dialogs.selectedThemeExecutionMode === 'trusted' ? 'themeTrustedRuntime.trustedEnable' : 'themePackage.runtimeEnable') }}
        </NButton>
      </div>
    </template>
  </NModal>
  <NModal :show="dialogs.createSpaceVisible" preset="dialog" :title="$t('spaceManage.createSpace')" :positive-text="$t('common.confirm')" :negative-text="$t('common.cancel')" :loading="dialogs.creatingSpace" @positive-click="dialogs.submitCreateSpace" @update:show="dialogs.setCreateSpaceVisible">
    <div data-testid="create-space-modal"><NInput :value="dialogs.spaceName" :placeholder="$t('spaceManage.newSpaceName')" maxlength="100" show-count @keyup.enter="dialogs.submitCreateSpace" @update:value="dialogs.setSpaceName" /></div>
  </NModal>
  <NModal :show="dialogs.groupCreateVisible" preset="dialog" :title="$t('spaceManage.addGroup')" :positive-text="$t('common.confirm')" :negative-text="$t('common.cancel')" :loading="dialogs.creatingGroup" @positive-click="dialogs.submitCreateGroup" @update:show="dialogs.setGroupCreateVisible">
    <div data-testid="create-group-modal"><NInput :value="dialogs.groupName" :placeholder="$t('spaceManage.groupName')" maxlength="100" @keyup.enter="dialogs.submitCreateGroup" @update:value="dialogs.setGroupName" /></div>
  </NModal>

  <!--
    The fixed utility stack from the pre-theme build, restored verbatim. Every
    length was measured off that build's own ruler screenshot at 1920x1080
    rather than eyeballed: 46x34 cells, four of them flush, 10px from the right
    edge and 50px up from the bottom. The icons are the same Iconify geometry,
    inlined because <SvgIcon> resolves them over the network.
  -->
  <div v-if="home.showFloatingBar" class="home-floating" role="group" :aria-label="$t('panelHome.floatingActions')" data-testid="home-floating-bar">
    <button type="button" class="home-floating-button" data-testid="floating-refresh-button" :title="$t('common.refresh')" :aria-label="$t('common.refresh')" @click="home.refreshCurrentSpace()">
      <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path fill="currentColor" d="M12 20q-3.35 0-5.675-2.325T4 12t2.325-5.675T12 4q1.725 0 3.3.712T18 6.75V5q0-.425.288-.712T19 4t.713.288T20 5v5q0 .425-.288.713T19 11h-5q-.425 0-.712-.288T13 10t.288-.712T14 9h3.2q-.8-1.4-2.187-2.2T12 6Q9.5 6 7.75 7.75T6 12t1.75 4.25T12 18q1.7 0 3.113-.862t2.187-2.313q.2-.35.563-.487t.737-.013q.4.125.575.525t-.025.75q-1.025 2-2.925 3.2T12 20"/></svg>
    </button>
    <button type="button" class="home-floating-button" data-testid="floating-top-button" :title="$t('spaceManage.backToTop')" :aria-label="$t('spaceManage.backToTop')" @click="home.scrollToTop">
      <svg viewBox="0 0 48 48" aria-hidden="true" focusable="false"><path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="4" d="M24.008 14.1V42M12 26l12-12l12 12M12 6h24"/></svg>
    </button>
    <button
      v-if="home.showNetworkSwitch"
      type="button"
      class="home-floating-button"
      :data-testid="home.networkMode === 'lan' ? 'floating-wan-button' : 'floating-lan-button'"
      :title="home.networkMode === 'lan' ? $t('panelHome.changeToWanModel') : $t('panelHome.changeToLanModel')"
      :aria-label="home.networkMode === 'lan' ? $t('panelHome.changeToWanModel') : $t('panelHome.changeToLanModel')"
      @click="home.toggleNetworkMode"
    >
      <!--
           Two glyphs, because the entry names the mode it switches *to*: the router
           while already on LAN, the globe while on WAN. This is what the pre-theme
           build did with `lan-outline-rounded` and `mdi:wan`.
      -->
      <svg v-if="home.networkMode === 'lan'" viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path fill="currentColor" d="M3 20v-3q0-.825.588-1.412T5 15h1v-2q0-.825.588-1.412T8 11h3V9h-1q-.825 0-1.412-.587T8 7V4q0-.825.588-1.412T10 2h4q.825 0 1.413.588T16 4v3q0 .825-.587 1.413T14 9h-1v2h3q.825 0 1.413.588T18 13v2h1q.825 0 1.413.588T21 17v3q0 .825-.587 1.413T19 22h-4q-.825 0-1.412-.587T13 20v-3q0-.825.588-1.412T15 15h1v-2H8v2h1q.825 0 1.413.588T11 17v3q0 .825-.587 1.413T9 22H5q-.825 0-1.412-.587T3 20m7-13h4V4h-4zM5 20h4v-3H5zm10 0h4v-3h-4zm0-3"/></svg>
      <svg v-else viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path fill="currentColor" d="M12 2a8 8 0 0 0-8 8c0 4.03 3 7.42 7 7.93V19h-1a1 1 0 0 0-1 1H2v2h7a1 1 0 0 0 1 1h4a1 1 0 0 0 1-1h7v-2h-7a1 1 0 0 0-1-1h-1v-1.07c4-.5 7-3.9 7-7.93a8 8 0 0 0-8-8m0 2s.74 1.28 1.26 3h-2.52C11.26 5.28 12 4 12 4m-2.23.43c-.27.5-.68 1.41-1.03 2.57H6.81C7.5 5.84 8.5 4.93 9.77 4.43m4.46.01c1.27.5 2.27 1.4 2.96 2.56h-1.93c-.35-1.16-.76-2.07-1.03-2.56M6.09 9h2.23c-.04.33-.07.66-.07 1s.03.67.07 1H6.09a5.6 5.6 0 0 1 0-2m4.23 0h3.36c.04.33.07.66.07 1s-.03.67-.07 1h-3.36c-.04-.33-.07-.66-.07-1s.03-.67.07-1m5.36 0h2.23a5.6 5.6 0 0 1 0 2h-2.23c.04-.33.07-.66.07-1s-.03-.67-.07-1m-8.87 4h1.93c.35 1.16.76 2.07 1.03 2.56c-1.27-.5-2.27-1.4-2.96-2.56m3.93 0h2.52c-.52 1.72-1.26 3-1.26 3s-.74-1.28-1.26-3m4.52 0h1.93c-.69 1.16-1.69 2.07-2.96 2.57c.27-.5.68-1.41 1.03-2.57"/></svg>
    </button>
    <button v-if="home.showSystemSettingsButton" type="button" class="home-floating-button" data-testid="system-settings-button" :title="$t('panelHome.systemSettings')" :aria-label="$t('panelHome.systemSettings')" @click="dialogs.setSettingModalShow(true)">
      <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><g fill="none"><path fill-rule="evenodd" clip-rule="evenodd" d="M6 3a3 3 0 0 0-3 3v2a3 3 0 0 0 3 3h2a3 3 0 0 0 3-3V6a3 3 0 0 0-3-3H6zm0 10a3 3 0 0 0-3 3v2a3 3 0 0 0 3 3h2a3 3 0 0 0 3-3v-2a3 3 0 0 0-3-3H6zm10 0a3 3 0 0 0-3 3v2a3 3 0 0 0 3 3h2a3 3 0 0 0 3-3v-2a3 3 0 0 0-3-3h-2zm0-10a3 3 0 0 0-3 3v2a3 3 0 0 0 3 3h2a3 3 0 0 0 3-3V6a3 3 0 0 0-3-3h-2z" fill="currentColor"/></g></svg>
    </button>
  </div>
</template>
