<script setup lang="ts">
import type { FormInst, FormRules } from 'naive-ui'
import { NButton, NCard, NDivider, NForm, NFormItem, NInput, NSelect, useDialog, useMessage } from 'naive-ui'
import { computed, onMounted, ref } from 'vue'
import { RoundCardModal, SvgIcon } from '../../common'
import { useAppStore, useAuthStore, usePanelState } from '@/store'
import { languageOptions } from '@/utils/defaultData'
import type { Language, Theme } from '@/store/modules/app/helper'
import { logout } from '@/api'
import { updateInfo, updatePassword } from '@/api/system/user'
import { updateLocalUserInfo } from '@/utils/cmn'
import { t } from '@/locales'
import { getInstalledThemes, getMyTheme, saveThemePreference } from '@/api/theme'
import { refreshMyTheme } from '@/hooks/useTheme'

// 使用导入的 ApiResponse 类型

const authStore = useAuthStore()
const appStore = useAppStore()
const panelState = usePanelState()
const ms = useMessage()
const dialog = useDialog()

const languageValue = ref(appStore.language)
const localizedLanguageOptions = computed(() => languageOptions.map(option => ({ ...option, label: option.key === 'auto' ? t('common.followBrowser') : option.label })))
const themeValue = ref(appStore.theme)
const selectedThemePackage = ref('')
const themeModeValue = ref<'custom' | 'follow-space'>('custom')
const themePackageOptions = ref<{ label: string; value: string }[]>([])
const themeModeOptions = computed(() => [
  { label: t('themeScope.modeCustom'), value: 'custom' },
  { label: t('themeScope.modeFollowSpace'), value: 'follow-space' },
])
const nickName = ref(authStore.userInfo?.name || '')
const isEditNickNameStatus = ref(false)
const formRef = ref<FormInst | null>(null)
const themeOptions: { label: string; key: string; value: Theme }[] = [
  { label: t('apps.userInfo.themeStyle.dark'), key: 'dark', value: 'dark' },
  { label: t('apps.userInfo.themeStyle.light'), key: 'light', value: 'light' },
  { label: t('apps.userInfo.themeStyle.auto'), key: 'Auto', value: 'auto' },
]
const updatePasswordModalState = ref({
  show: false,
  loading: false,
  form: {
    password: '',
    oldPassword: '',
    confirmPassword: '',
  },
})

const updatePasswordModalFormRules: FormRules = {
  oldPassword: {
    required: true,
    trigger: 'blur',
    min: 6,
    max: 20,
    message: t('adminSettingUsers.formRules.passwordLimit'),
  },
  password: {
    required: true,
    trigger: 'blur',
    min: 6,
    max: 20,
    message: t('adminSettingUsers.formRules.passwordLimit'),
  },
  confirmPassword: {
    required: true,
    trigger: 'blur',
    min: 6,
    max: 20,
    message: t('adminSettingUsers.formRules.passwordLimit'),
  },
}

async function logoutApi() {
  await logout()
  authStore.removeStorage()
  panelState.removeState()
  appStore.removeStorage()
  ms.success(t('settingUserInfo.logoutSuccess'))
  location.reload()
}

function handleSaveInfo() {
  updateInfo(nickName.value).then(({ code, msg }) => {
    if (code === 0) {
      updateLocalUserInfo()
      isEditNickNameStatus.value = false
    }
    else {
      ms.error(`${t('common.editFail')}:${msg}`)
    }
  })
}

function handleUpdatePassword(e: MouseEvent) {
  e.preventDefault()
  formRef.value?.validate((errors) => {
    if (errors) {
      console.log(errors)
      return
    }

    if (updatePasswordModalState.value.form.password !== updatePasswordModalState.value.form.confirmPassword) {
      ms.error(t('settingUserInfo.confirmPasswordInconsistentMsg'))
      return
    }
    updatePasswordModalState.value.loading = true
    updatePassword(updatePasswordModalState.value.form.oldPassword, updatePasswordModalState.value.form.password).then(({ code, msg }) => {
      if (code === 0) {
        // 成功
        updatePasswordModalState.value.show = false
        ms.success(t('common.success'))
      }
    }).finally(() => {
      updatePasswordModalState.value.loading = false
    }).catch(() => {
      ms.error(t('common.serverError'))
    })
  })
}

function handleLogout() {
  dialog.warning({
    title: t('common.warning'),
    content: t('settingUserInfo.confirmLogoutText'),
    positiveText: t('common.confirm'),
    negativeText: t('common.cancel'),
    onPositiveClick: () => {
      logoutApi()
    },
  })
}

function handleChangeLanuage(value: Language) {
  languageValue.value = value
  appStore.setLanguage(value)
  location.reload()
}

function handleChangeTheme(value: Theme) {
  themeValue.value = value
  appStore.setTheme(value)
  saveThemePreference({ packageId: selectedThemePackage.value, mode: value })
}

async function loadThemeSettings() {
  try {
    const [installed, mine] = await Promise.all([getInstalledThemes(), getMyTheme()])
    if (installed.code === 0) {
      themePackageOptions.value = [
        { label: t('themePackage.instanceDefault'), value: '' },
        ...installed.data.map(item => ({ label: item.name, value: item.id })),
      ]
    }
    if (mine.code === 0) {
      selectedThemePackage.value = mine.data.preference.packageId || ''
      themeValue.value = mine.data.preference.mode
      themeModeValue.value = mine.data.preference.themeMode || 'custom'
    }
  }
  catch {
    // Theme selection remains on the server's current default when unavailable.
  }
}

async function handleChangeThemePackage(packageId: string) {
  selectedThemePackage.value = packageId
  const result = await saveThemePreference({ packageId, mode: themeValue.value, themeMode: themeModeValue.value })
  if (result.code === 0) {
    await refreshMyTheme(appStore)
    ms.success(t('themePackage.saved'))
  }
  else {
    ms.error(result.msg)
  }
}

async function handleChangeThemeMode(value: 'custom' | 'follow-space') {
  themeModeValue.value = value
  const result = await saveThemePreference({ packageId: selectedThemePackage.value, mode: themeValue.value, themeMode: value })
  if (result.code === 0) {
    await refreshMyTheme(appStore)
    ms.success(t('themeScope.saved'))
  }
  else {
    ms.error(result.msg)
  }
}

onMounted(loadThemeSettings)

</script>

<template>
  <div class="theme-page p-2 h-full">
    <NCard style="border-radius:10px" size="small">
      <div>
        <div class="text-slate-500 font-bold">
          {{ $t('spaceManage.userEmail') }}
        </div>
          {{ authStore.userInfo?.mail }}
      </div>

      <div class="mt-[10px]">
        <div class="text-slate-500 font-bold">
          {{ $t('common.nickName') }}
        </div>

        <div v-if="!isEditNickNameStatus">
          {{ authStore.userInfo?.name }}

          <NButton size="small" text type="info" @click="isEditNickNameStatus = !isEditNickNameStatus">
            {{ $t('common.edit') }}
          </NButton>
        </div>

        <div v-else class="flex items-center">
          <div class="max-w-[150px]">
            <NInput v-model:value="nickName" type="text" :placeholder="$t('common.inputPlaceholder')" />
          </div>
          <NButton size="small" quaternary type="info" @click="handleSaveInfo">
            {{ $t('common.save') }}
          </NButton>
        </div>
      </div>

      <div class="mt-[10px]">
        <div class="text-slate-500 font-bold">
          {{ $t('common.password') }}
        </div>

        <NButton size="small" text type="info" @click="updatePasswordModalState.show = !updatePasswordModalState.show">
          {{ $t('settingUserInfo.updatePassword') }}
        </NButton>
      </div>

      <NDivider style="margin: 10px 0;" dashed />

      <div class="mt-[10px]">
        <div class="text-slate-500 font-bold">
          {{ $t('common.language') }}
        </div>
        <div class="max-w-[200px]">
          <NSelect v-model:value="languageValue" :options="localizedLanguageOptions" @update-value="handleChangeLanuage" />
        </div>
      </div>

      <div class="mt-[10px]">
        <div class="text-slate-500 font-bold">
          {{ $t('apps.userInfo.theme') }}
        </div>
        <div class="max-w-[200px]">
        <NSelect v-model:value="themeValue" :options="themeOptions" @update-value="handleChangeTheme" />
        <div class="mt-3">
          <div class="mb-1">{{ $t('themeScope.mode') }}</div>
          <NSelect v-model:value="themeModeValue" :options="themeModeOptions" @update-value="handleChangeThemeMode" />
        </div>
        <div class="mt-3">
          <div class="mb-1">{{ $t('themePackage.label') }}</div>
          <NSelect v-model:value="selectedThemePackage" :options="themePackageOptions" :disabled="themeModeValue === 'follow-space'" @update-value="handleChangeThemePackage" />
        </div>
        </div>
      </div>

      <NDivider style="margin: 10px 0;" dashed />

    </NCard>

    <NCard style="border-radius:10px" class="mt-[10px]" size="small">
      <NButton size="small" text type="error" @click="handleLogout">
        <template #icon>
          <SvgIcon icon="tabler:logout" />
        </template>
        {{ $t('settingUserInfo.logout') }}
      </NButton>
    </NCard>

    <RoundCardModal v-model:show="updatePasswordModalState.show" size="small" preset="card" style="width: 400px" :title="$t('settingUserInfo.updatePassword')">
      <NForm ref="formRef" :model="updatePasswordModalState.form" :rules="updatePasswordModalFormRules">
        <NFormItem path="oldPassword" :label="$t('settingUserInfo.oldPassword')">
          <NInput v-model:value="updatePasswordModalState.form.oldPassword" :maxlength="20" type="password" :placeholder="$t('settingUserInfo.oldPassword')" />
        </NFormItem>

        <NFormItem path="password" :label="$t('settingUserInfo.newPassword')">
          <NInput v-model:value="updatePasswordModalState.form.password" :maxlength="20" type="password" :placeholder="$t('settingUserInfo.newPassword')" />
        </NFormItem>

        <NFormItem path="confirmPassword" :label="$t('settingUserInfo.confirmPassword')">
          <NInput v-model:value="updatePasswordModalState.form.confirmPassword" :maxlength="20" type="password" :placeholder="$t('settingUserInfo.confirmPassword')" />
        </NFormItem>
      </NForm>

      <template #footer>
        <div class="float-right">
          <NButton type="success" size="small" :loading="updatePasswordModalState.loading" @click="handleUpdatePassword">
            {{ $t('common.save') }}
          </NButton>
        </div>
      </template>
    </RoundCardModal>
  </div>
</template>
