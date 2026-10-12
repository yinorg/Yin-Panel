<script setup lang="ts">
import type { FormInst, FormRules } from 'naive-ui'
import { NButton, NCard, NDivider, NForm, NFormItem, NInput, NSelect, useDialog, useMessage } from 'naive-ui'
import { computed, ref } from 'vue'
import { RoundCardModal, SvgIcon } from '../../common'
import { useAppStore, useAuthStore, usePanelState } from '@/store'
import { languageOptions } from '@/utils/defaultData'
import type { Language } from '@/store/modules/app/helper'
import { logout } from '@/api'
import { updateInfo, updatePassword } from '@/api/system/user'
import { updateLocalUserInfo } from '@/utils/cmn'
import { t } from '@/locales'
import { set as setUserConfig } from '@/api/panel/userConfig'

const authStore = useAuthStore()
const appStore = useAppStore()
const panelState = usePanelState()
const ms = useMessage()
const dialog = useDialog()

const languageValue = ref(appStore.language)
const localizedLanguageOptions = computed(() => languageOptions.map(option => ({ ...option, label: option.key === 'auto' ? t('common.followBrowser') : option.label })))
const nickName = ref(authStore.userInfo?.name || '')
const isEditNickNameStatus = ref(false)
const formRef = ref<FormInst | null>(null)
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
    updatePassword(updatePasswordModalState.value.form.oldPassword, updatePasswordModalState.value.form.password).then(({ code }) => {
      if (code === 0) {
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

async function handleChangeLanuage(value: Language) {
  languageValue.value = value
  appStore.setLanguage(value)
  try {
    // The choice belongs to the account, so it reaches the user on their next
    // device rather than living only in this browser's storage.
    await setUserConfig({ language: value })
  }
  catch {
    // The local choice stays in effect when the save fails.
  }
  location.reload()
}
</script>

<template>
  <div class="theme-page p-2 h-full">
    <NCard style="border-radius:10px" size="small">
      <div>
        <div class="yin-text-muted font-bold">
          {{ $t('spaceManage.userEmail') }}
        </div>
        {{ authStore.userInfo?.mail }}
      </div>

      <div class="mt-[10px]">
        <div class="yin-text-muted font-bold">
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
        <div class="yin-text-muted font-bold">
          {{ $t('common.password') }}
        </div>

        <NButton size="small" text type="info" @click="updatePasswordModalState.show = !updatePasswordModalState.show">
          {{ $t('settingUserInfo.updatePassword') }}
        </NButton>
      </div>

      <NDivider style="margin: 10px 0;" dashed />

      <div>
        <div class="yin-text-muted font-bold">
          {{ $t('common.language') }}
        </div>
        <div class="max-w-[200px]">
          <NSelect v-model:value="languageValue" :options="localizedLanguageOptions" @update-value="handleChangeLanuage" />
        </div>
        <div class="mt-1 text-xs opacity-70">
          {{ $t('apps.account.languageHint') }}
        </div>
      </div>
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
