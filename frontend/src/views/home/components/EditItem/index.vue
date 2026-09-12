<script setup lang="ts">
import { computed, defineEmits, defineProps, ref, watch } from 'vue'
import type { FormInst, FormRules } from 'naive-ui'
import { NButton, NForm, NFormItem, NGrid, NGridItem, NInput, NInputGroup, NModal, NSelect, useMessage } from 'naive-ui'
import IconEditor from './IconEditor.vue'
import { edit, getSiteFavicon } from '../../../../api/panel/itemIcon'
import { getGroups } from '../../../../api/panel/space'
import { createItem, updateItem } from '../../../../api/panel/space'
import { t } from '../../../../locales'
import { useAuthStore } from '../../../../store'

interface Props {
  visible: boolean
  itemInfo: Panel.Info | null
  itemGroupId?: number
  spaceId?: number
}

const props = defineProps<Props>()
const emit = defineEmits<Emit>()
const ms = useMessage()
const submitLoading = ref(false)
const getIconLoading = ref([false, false])
const itemIconGroupOptions = ref<{
  label: string
  value: number
}[]>([])

const restoreDefault: Panel.Info = {
  icon: null,
  title: '',
  url: '',
  lanUrl: '',
  description: '',
  openMethod: 2,
}

interface Emit {
  (e: 'update:visible', visible: boolean): void
  (e: 'done', item: Panel.Info): void// 创建完成
}

const model = ref<Panel.Info>(props.itemInfo ? { ...props.itemInfo } : { ...restoreDefault })
const formRef = ref<FormInst | null>(null)

const rules: FormRules = {
  title: {
    required: true,
    trigger: 'blur',
    message: t('form.required'),
  },
  url: {
    required: true,
    trigger: 'blur',
    type: 'string',
    message: t('form.required'),
  },
  // itemIconGroupId: {
  //   required: true,
  //   trigger: ['blur', 'change'],
  //   message: t('form.required'),
  // },
}

const options = [
  {
    default: true,
    label: t('iconItem.currentPageOpen'),
    value: 1,
  },
  {
    label: t('iconItem.newWindowOpen'),
    value: 2,
  },
  {
    label: t('iconItem.currentPageLayerOpen'),
    value: 3,
  },
]

// 更新值父组件传来的值
const show = computed({
  get: () => props.visible,
  set: (visible: boolean) => {
    emit('update:visible', visible)
  },
})

async function editApi() {
  submitLoading.value = true
  try {
    if (!model.value.id && !model.value.icon?.src && model.value.url) {
      const fetched = await getIconByUrl(model.value.url, 0, false)
      if (!fetched) {
        const libraryIcon = await getPublicLibraryIcon(model.value.title)
        if (libraryIcon)
          model.value.icon = libraryIcon
      }
      if (!model.value.icon?.src && model.value.icon?.itemType !== 2)
        model.value.icon = createTextIcon(model.value.title)
    }
    const { code, data, msg } = await (props.spaceId
      ? (model.value.id ? updateItem<Panel.ItemInfo>(props.spaceId, model.value.id, model.value) : createItem<Panel.ItemInfo>(props.spaceId, model.value))
      : edit<Panel.ItemInfo>(model.value))
    if (code === 0) {
      show.value = false
      model.value = { ...restoreDefault }

      emit('done', data)
    }
    else {
      ms.error(`${t('common.saveFail')}:${msg}`)
    }
  }
  catch (error) {
    ms.error(t('common.saveFail'))
  }
  submitLoading.value = false
}

async function getPublicLibraryIcon(title: string): Promise<Panel.ItemIcon | null> {
  try {
    const query = encodeURIComponent(title.trim())
    const search = await fetch(`https://api.iconify.design/search?query=${query}&limit=1`)
    if (!search.ok) return null
    const result = await search.json() as { icons?: string[] }
    const name = result.icons?.[0]
    if (!name) return null
    const image = await fetch(`https://api.iconify.design/${name}.svg`)
    if (!image.ok) return null
    const blob = await image.blob()
    const form = new FormData()
    form.append('imgfile', new File([blob], `${name.replace('/', '-')}.svg`, { type: 'image/svg+xml' }))
    const upload = await fetch('/api/file/uploadImg', { method: 'POST', headers: { Authorization: `Bearer ${useAuthStore().token}` }, body: form })
    if (!upload.ok) return null
    const response = await upload.json()
    if (response.code !== 0) return null
    return { itemType: 2, src: response.data.imageUrl, fileName: response.data.fileName }
  } catch {
    return null
  }
}

const handleValidateButtonClick = (e: MouseEvent) => {
  e.preventDefault()
  formRef.value?.validate((errors) => {
    if (!errors)
      editApi()
  })
}

function createTextIcon(title: string): Panel.ItemIcon {
  const chinese = title.match(/[\u3400-\u9fff]/g)?.join('').slice(0, 5) || ''
  const english = title.match(/[A-Za-z]/g)?.join('').slice(0, 8) || ''
  const text = chinese || english || title.trim().slice(0, 5) || '?'
  return { itemType: 1, text, backgroundColor: '#2a2a2a6b' }
}

async function getIconByUrl(url: string, loadingIndex: number, showError = true): Promise<boolean> {
  getIconLoading.value[loadingIndex] = true
  try {
    const { code, data } = await getSiteFavicon<{ iconUrl: string, fileName: string }>(url)
    if (code === 0) {
      model.value.icon = {
        itemType: 2,
        src: data.iconUrl,
        fileName: data.fileName,
      }
    }
    else {
      if (showError) ms.error(t('iconItem.geticonFail'))
    }
  }
  catch (error) {
    if (showError) ms.error(t('iconItem.geticonFail'))
  }
  getIconLoading.value[loadingIndex] = false
  return !!model.value.icon?.src
}

watch(() => props.visible, (newValue) => {
  if (newValue === true) {
    model.value = props.itemInfo ? { ...props.itemInfo } : { ...restoreDefault }
    if (props.itemGroupId)
      model.value.itemIconGroupId = props.itemGroupId
  }

  getGroupListOptions()
})

function getGroupListOptions() {
  if (!props.spaceId) {
    itemIconGroupOptions.value = []
    return
  }
  getGroups<{ code: number; data: Panel.ItemIconGroup[] }>(props.spaceId).then(({ data, code, msg }) => {
    if (code === 0) {
      itemIconGroupOptions.value = []

      const list = data as Panel.ItemIconGroup[]
      for (let i = 0; i < list.length; i++) {
        const element = list[i]
        if (i === 0 && !model.value.itemIconGroupId) {
          model.value.itemIconGroupId = element.id
          restoreDefault.itemIconGroupId = element.id
        }

        itemIconGroupOptions.value.push({
          value: element.id as number,
          label: element.title as string,
        })
      }
    }
    else {
      ms.error(`${t('iconItem.getGroupFail')}:${msg}`)
    }
  })
}
</script>

<template>
  <NModal v-model:show="show" preset="card" size="small" style="width: 600px;max-width:calc(100vw - 24px);max-height:90vh;border-radius:1rem;overflow:auto;" :title="itemInfo ? t('iconItem.edit') : t('iconItem.add')">
    <div class="h-[600px] overflow-auto p-[5px]">
      <NForm ref="formRef" :model="model" :rules="rules">
        <NGrid cols="2" :x-gap="10" item-responsive>
          <NGridItem span="2 500:1">
            <NFormItem path="itemIconGroupId" :label="t('iconItem.iconGroup')">
              <NSelect v-model:value="model.itemIconGroupId" :options="itemIconGroupOptions" />
            </NFormItem>
          </NGridItem>
          <NGridItem span="2 500:1">
            <NFormItem path="title" :label="$t('common.title')">
              <NInput v-model:value="model.title" type="text" show-count :maxlength="20" />
            </NFormItem>
          </NGridItem>
        </NGrid>

        <NFormItem path="icon" :label="$t('common.icon')">
          <IconEditor v-model:item-icon="model.icon" />
        </NFormItem>
        <NFormItem path="url" :label="$t('iconItem.url')">
          <!-- <NSelect :style="{ width: '100px' }" :options="urlProtocolOptions" /> -->
          <NInputGroup>
            <NInput v-model:value="model.url" type="text" :maxlength="1000" placeholder="http(s)://" />
            <NButton :disabled="!model.url" :loading="getIconLoading[0]" @click="getIconByUrl(model.url, 0)">
              {{ $t('iconItem.getIcon') }}
            </NButton>
          </NInputGroup>
        </NFormItem>
        <NFormItem path="lanUrl" :label="$t('iconItem.lanUrl')">
          <NInputGroup>
            <NInput v-model:value="model.lanUrl" type="text" :maxlength="1000" :placeholder="$t('iconItem.lanUrlInputPlaceholder')" />
            <NButton :disabled="!model.lanUrl" :loading="getIconLoading[1]" @click="getIconByUrl(model.lanUrl || '', 1)">
              {{ $t('iconItem.getIcon') }}
            </NButton>
          </NInputGroup>
        </NFormItem>
        <NFormItem path="description" :label="$t('common.description')">
          <NInput v-model:value="model.description" type="text" show-count :maxlength="100" />
        </NFormItem>
        <NFormItem path="openMethod" :label="$t('iconItem.openMethod')">
          <NSelect v-model:value="model.openMethod" :options="options" />
        </NFormItem>
      </NForm>
    </div>

    <template #footer>
      <NButton type="success" :loading="submitLoading" style="float: right;" @click="handleValidateButtonClick">
        {{ $t('common.save') }}
      </NButton>
    </template>
  </NModal>
</template>
