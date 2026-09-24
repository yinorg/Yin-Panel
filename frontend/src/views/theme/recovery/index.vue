<script setup lang="ts">
import { computed, ref } from 'vue'
import { NButton } from 'naive-ui'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { disableThemeSafeMode, enableThemeSafeMode, isThemeSafeMode } from '@/theme/recovery/safeMode'
import { getThemePreference, saveThemePreference } from '@/api/theme'

const { t } = useI18n()
const router = useRouter()
const safeMode = computed(() => isThemeSafeMode())
const restoring = ref(false)
const restoreFailed = ref(false)

function startSafeMode() {
  enableThemeSafeMode()
  window.location.assign('/')
}

function exitSafeMode() {
  disableThemeSafeMode()
  window.location.assign('/')
}

function returnHome() {
  void router.push('/')
}

async function restoreYin() {
  if (restoring.value) return
  restoring.value = true
  restoreFailed.value = false
  try {
    const current = await getThemePreference()
    if (current.code !== 0) throw new Error('Could not load the current theme preference')
    const saved = await saveThemePreference({ packageId: 'org.yin.default', mode: current.data.mode })
    if (saved.code !== 0) throw new Error('Could not restore the Yin theme')
    enableThemeSafeMode()
    window.location.assign('/')
  }
  catch {
    restoreFailed.value = true
  }
  finally {
    restoring.value = false
  }
}
</script>

<template>
  <main class="theme-recovery" data-testid="theme-recovery">
    <section class="theme-recovery__content">
      <p class="theme-recovery__eyebrow">
        Yin-Panel
      </p>
      <h1>{{ t('themeRecovery.title') }}</h1>
      <p class="theme-recovery__description">
        {{ t('themeRecovery.description') }}
      </p>
      <p v-if="safeMode" class="theme-recovery__status" role="status" data-testid="theme-safe-mode-active">
        {{ t('themeRecovery.active') }}
      </p>
      <p v-if="restoreFailed" class="theme-recovery__error" role="alert" data-testid="theme-recovery-error">
        {{ t('themeRecovery.restoreFailed') }}
      </p>
      <div class="theme-recovery__actions">
        <NButton v-if="!safeMode" type="primary" data-testid="theme-safe-mode-start" @click="startSafeMode">
          {{ t('themeRecovery.start') }}
        </NButton>
        <NButton v-else type="primary" data-testid="theme-safe-mode-exit" @click="exitSafeMode">
          {{ t('themeRecovery.exit') }}
        </NButton>
        <NButton type="primary" :loading="restoring" data-testid="theme-recovery-restore-yin" @click="restoreYin">
          {{ t('themeRecovery.restoreYin') }}
        </NButton>
        <NButton quaternary data-testid="theme-recovery-home" @click="returnHome">
          {{ t('themeRecovery.home') }}
        </NButton>
      </div>
    </section>
  </main>
</template>

<style scoped>
.theme-recovery {
  display: grid;
  min-height: 100%;
  padding: 32px;
  place-items: center;
  color: var(--yin-text, #20242b);
  background: var(--yin-canvas, #f5f6f8);
}

.theme-recovery__content {
  width: min(100%, 560px);
}

.theme-recovery__eyebrow {
  margin: 0 0 12px;
  color: var(--yin-primary, #2675d8);
  font-size: 13px;
  font-weight: 700;
}

h1 {
  margin: 0;
  font-size: 28px;
  line-height: 1.25;
}

.theme-recovery__description,
.theme-recovery__status {
  margin: 16px 0 0;
  line-height: 1.6;
}

.theme-recovery__status {
  color: var(--yin-success, #28784a);
}

.theme-recovery__error {
  color: var(--yin-danger, #a12627);
}

.theme-recovery__actions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 24px;
}

@media (max-width: 480px) {
  .theme-recovery {
    padding: 24px;
  }
}
</style>
