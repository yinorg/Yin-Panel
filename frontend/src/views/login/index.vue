<script setup lang="ts">
import { NButton, NCard, NForm, NFormItem, NGradientText, NInput, NSelect, useMessage, NDivider } from 'naive-ui'
import { computed, ref, onMounted } from 'vue'
import { login } from '../../api'
import { useAppStore, useAuthStore } from '../../store'
import SvgIcon from '../../components/common/SvgIcon/index.vue'
import SvgIconOnline from '../../components/common/SvgIconOnline/index.vue'
import { router } from '../../router'
import { t } from '../../locales'
import { languageOptions } from '../../utils/defaultData'
import type { Language } from '../../store/modules/app/helper'
import service from '../../utils/request/axios'

const authStore = useAuthStore()
const appStore = useAppStore()
const ms = useMessage()
const loading = ref(false)
const languageValue = ref<Language>(appStore.language)
const localizedLanguageOptions = computed(() => languageOptions.map(option => ({ ...option, label: option.key === 'auto' ? t('common.followBrowser') : option.label })))
const oauthEnabled = ref(false)
const oauthProviders = ref<string[]>([])
const oauthLoading = ref(false)
const loadingProvider = ref<string>('')

const oauthProviderMeta: Record<string, { icon: string; label: string }> = {
  authentik: { icon: 'mdi:shield-account', label: 'Authentik' },
}

const form = ref<Login.LoginReqest>({
  mail: '',
  password: '',
})

onMounted(async () => {
  try {
    // 获取OAuth配置
    const res = await service.get('oauth/config')
    if (res.data.code === 0) {
      oauthEnabled.value = res.data.data.enabled
      oauthProviders.value = res.data.data.providers || []
    }
  } catch (error) {
    console.error('Failed to fetch OAuth config:', error)
  }

})

const loginPost = async () => {
  loading.value = true
  try {
    const res = await login<Login.LoginResponse>(form.value)
    if (res.code === 0) {
      authStore.setToken(res.data.token)
      authStore.setUserInfo(res.data)
      authStore.saveStorage()

      setTimeout(() => {
        ms.success(`Hi ${res.data.name},${t('login.welcomeMessage')}`)
        loading.value = false
        router.push({ path: '/' })
      }, 500)
    }
    else {
      loading.value = false
    }
  }
  catch (error) {
    loading.value = false
    // 请检查网络或者服务器错误
    console.log(error)
  }
}

function handleSubmit() {
  // 点击登录按钮触发
  loginPost()
}

function handleChangeLanuage(value: Language) {
  languageValue.value = value
  appStore.setLanguage(value)
}

function handleOAuthLogin(provider: string) {
  oauthLoading.value = true
  loadingProvider.value = provider
  
  // 构建OAuth URL
  const oauthUrl = `/api/oauth/${provider}`
  
  // 直接在当前窗口打开OAuth登录页面
  window.location.href = oauthUrl
}

function getProviderIcon(provider: string) {
  return oauthProviderMeta[provider.toLowerCase()]?.icon || `mdi:${provider}`
}

function getProviderLabel(provider: string) {
  return oauthProviderMeta[provider.toLowerCase()]?.label || `${provider.charAt(0).toUpperCase()}${provider.slice(1)}`
}

</script>

<template>
  <div class="login-container">
    <NCard class="login-card" style="border-radius: 20px;">
      <div class="mb-5 flex items-center justify-end">
        <div class="mr-2">
          <SvgIcon icon="ion-language" style="width: 20;height: 20;" />
        </div>
        <div class="min-w-[100px]">
          <NSelect v-model:value="languageValue" size="small" :options="localizedLanguageOptions" @update-value="handleChangeLanuage" />
        </div>
      </div>

      <div class="login-title" data-lcp="brand">
        <NGradientText :size="30" type="success" class="!font-bold">
          {{ $t('common.appName') }}
        </NGradientText>
      </div>
      <NForm :model="form" label-width="100px" @keydown.enter="handleSubmit">
        <NFormItem>
          <NInput v-model:value="form.mail" :placeholder="$t('login.usernamePlaceholder')">
            <template #prefix>
              <SvgIcon icon="ph:user-bold" />
            </template>
          </NInput>
        </NFormItem>

        <NFormItem>
          <NInput v-model:value="form.password" type="password" :placeholder="$t('login.passwordPlaceholder')">
            <template #prefix>
              <SvgIcon icon="mdi:password-outline" />
            </template>
          </NInput>
        </NFormItem>

        <NFormItem style="margin-top: 10px">
          <NButton type="primary" block :loading="loading" @click="handleSubmit">
            {{ $t('login.loginButton') }}
          </NButton>
        </NFormItem>

        <!-- OAuth登录按钮 -->
        <div v-if="oauthEnabled && oauthProviders.length > 0">
          <NDivider>{{ $t('login.thirdPartyLogin') }}</NDivider>
          <div class="oauth-buttons flex flex-col gap-2">
            <NButton 
              v-for="provider in oauthProviders"
              :key="provider"
              quaternary 
              class="oauth-button" 
              :loading="oauthLoading && loadingProvider === provider"
              :disabled="oauthLoading && loadingProvider !== provider"
              @click="handleOAuthLogin(provider)"
            >
              <template #icon>
                <SvgIconOnline :icon="getProviderIcon(provider)" />
              </template>
              {{ getProviderLabel(provider) }}
            </NButton>
          </div>
        </div>
      </NForm>
    </NCard>
  </div>
</template>

  <style>
    .login-container {
        padding: 20px;
        display: flex;
        justify-content: center;
        align-items: center;
        height: 100vh;
        background-color: #f2f6ff;
    }

    /* 夜间模式 */
    .dark .login-container{
      background-color: rgb(43, 43, 43);
    }

    @media (min-width: 600px) {
        .login-card {
            width: auto;
            margin: 0px 10px;
        }
        .login-button {
            width: 100%;
        }
    }

    .login-card {
        margin: 20px;
        width: min(100%, 400px);
        min-width: 0;
    }

  .login-title{
    text-align: center;
    margin: 20px;
  }

  .oauth-buttons {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 10px;
  }

  .oauth-button {
    min-width: 100px;
    width: 100%;
  }
  </style>
