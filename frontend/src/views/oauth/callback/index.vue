<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { NButton, NCard, NGradientText, NResult, NSpin, useMessage } from 'naive-ui'
import { useAuthStore } from '../../../store'
import { router } from '../../../router'
import { getUser } from '../../../api/system/user'
import { t } from '../../../locales'

const authStore = useAuthStore()
const message = useMessage()
const status = ref<'loading' | 'success' | 'error'>('loading')
const error = ref('')

const errorMessage: Record<string, string> = {
  provider_denied: 'oauthProviderDenied',
  provider_unsupported: 'oauthCallbackFailed',
  state_invalid: 'oauthStateInvalid',
  missing_code: 'oauthMissingCode',
  callback_failed: 'oauthCallbackFailed',
  token_failed: 'oauthCallbackFailed',
}

onMounted(async () => {
  const callbackURL = new URL(window.location.href)
  const token = callbackURL.searchParams.get('token')
  const reason = callbackURL.searchParams.get('error')
  window.history.replaceState({}, document.title, callbackURL.pathname)

  if (!token || reason) {
    status.value = 'error'
    error.value = t(`login.${errorMessage[reason || ''] || 'oauthCallbackFailed'}`)
    return
  }

  authStore.setToken(token)
  authStore.saveStorage()
  try {
    const { data } = await getUser()
    if (!data) throw new Error('user information is missing')
    authStore.setUserInfo(data)
    authStore.saveStorage()
    status.value = 'success'
    message.success(`Hi ${data.name}, ${t('login.welcomeMessage')}`)
    await new Promise(resolve => setTimeout(resolve, 400))
    await router.replace('/')
  } catch {
    authStore.setToken('')
    authStore.saveStorage()
    status.value = 'error'
    error.value = t('login.oauthCallbackFailed')
  }
})
</script>

<template>
  <div class="oauth-callback">
    <NCard class="oauth-callback-card">
      <div class="oauth-title">
        <NGradientText :size="30" type="success" class="!font-bold">
          {{ $t('common.appName') }}
        </NGradientText>
      </div>
      <div v-if="status === 'loading'" class="oauth-loading">
        <NSpin size="large" />
        <p>{{ $t('login.oauthProcessing') }}</p>
      </div>
      <NResult v-else-if="status === 'success'" status="success" :title="$t('login.oauthSuccess')" />
      <NResult v-else status="error" :title="$t('login.oauthFailed')" :description="error">
        <template #footer><NButton type="primary" @click="router.replace('/login')">{{ $t('login.oauthBackToLogin') }}</NButton></template>
      </NResult>
    </NCard>
  </div>
</template>

<style scoped>
.oauth-callback {
  min-height: 100vh;
  padding: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: #f2f6ff;
}

.dark .oauth-callback {
  background-color: rgb(43, 43, 43);
}

.oauth-callback-card {
  margin: 20px;
  width: min(100%, 400px);
  min-width: 0;
}

.oauth-title {
  margin: 20px;
  text-align: center;
}

.oauth-loading {
  min-height: 150px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 18px;
}

.oauth-loading p {
  margin: 0;
}
</style>
