<script setup lang="ts">
import { NButton, NColorPicker, NInput, NRadio, NUpload, useMessage } from 'naive-ui'
import type { UploadCustomRequestOptions } from 'naive-ui'
import { computed } from 'vue'
import { ItemIcon } from '../../../../components/common'
import { t } from '../../../../locales'

const props = defineProps<{
  itemIcon: Panel.ItemIcon | null
  uploadImage: (file: File) => Promise<{ imageUrl: string; fileName: string }>
}>()
const emit = defineEmits<{
  (e: 'update:itemIcon', visible: Panel.ItemIcon): void // 定义修改父组件（prop内）的值的事件
}>()
const message = useMessage()

// 默认图标背景色
const defautSwatchesBackground = [
  '#00000000',
  '#000000',
  '#ffffff',
  '#18A058',
  '#2080F0',
  '#F0A020',
  'rgba(208, 48, 80, 1)',
  '#C418D1FF',
]

const initData: Panel.ItemIcon = {
  itemType: 4,
  backgroundColor: '#2a2a2a6b',
}

const itemIconInfo = computed({
  get() {
    const v = {
      ...initData,
      ...props.itemIcon,
      backgroundColor: props.itemIcon?.backgroundColor || initData.backgroundColor,
    }
    return v
  },
  set() {
    handleChange()
  },
})

function handleIconTypeRadioChange(type: number) {
  // checkedValueRef.value = type
  itemIconInfo.value.itemType = type
  handleChange()
}

function handleChange() {
  emit('update:itemIcon', itemIconInfo.value || null)
}

function handleResetBackgroundColor() {
  itemIconInfo.value.backgroundColor = initData.backgroundColor
  handleChange()
}

async function handleUpload({ file, onFinish, onError }: UploadCustomRequestOptions) {
  if (!file.file) {
    onError()
    return
  }
  try {
    const uploaded = await props.uploadImage(file.file)
    itemIconInfo.value.src = uploaded.imageUrl
    itemIconInfo.value.fileName = uploaded.fileName
    emit('update:itemIcon', itemIconInfo.value || null)
    onFinish()
  }
  catch {
    message.error(t('common.uploadFail'))
    onError()
  }
}
</script>

<template>
  <div>
    <div class="mb-[10px]">
      <NRadio :checked="itemIconInfo.itemType === 4" :value="4" name="iconType" @change="handleIconTypeRadioChange(4)">
        {{ $t('iconItem.auto') }}
      </NRadio>
      <NRadio
        :checked="itemIconInfo.itemType === 1 "
        :value="1"
        name="iconType"
        @change="handleIconTypeRadioChange(1)"
      >
        {{ $t('common.text') }}
      </NRadio>

      <NRadio
        :checked="itemIconInfo.itemType === 2"
        :value="2"
        name="iconType"
        @change="handleIconTypeRadioChange(2)"
      >
        {{ $t('common.image') }}
      </NRadio>

      <NRadio
        :checked="itemIconInfo.itemType === 3"
        :value="3"
        name="iconType"
        @change="handleIconTypeRadioChange(3)"
      >
        {{ $t('iconItem.onlineIcon') }}
      </NRadio>
    </div>

    <div class=" h-[100px]">
      <div class="flex">
        <div>
          <div class="border rounded-2xl bg-slate-200 overflow-hidden rounded-2xl transparent-grid">
            <ItemIcon :item-icon="itemIconInfo" />
          </div>
        </div>
        <!-- 文字 -->
        <div class="ml-[20px]">
          <!-- <NImage :src="model.icon" preview-disabled /> -->
          <div v-if="itemIconInfo.itemType === 1">
            <NInput v-model:value="itemIconInfo.text" class="mb-[5px]" size="small" type="text" @input="handleChange" />
          </div>

          <div v-if="itemIconInfo.itemType === 3">
            <div>
              <NInput v-model:value="itemIconInfo.text" class="mb-[5px]" size="small" type="text" :placeholder="$t('iconItem.inputIconName')" @input="handleChange" />

              <NButton quaternary type="info">
                <a target="_blank" href="https://icon-sets.iconify.design/">{{ $t('iconItem.onlineIconLibrary') }}</a>
              </NButton>
            </div>
          </div>

          <!-- 图片 -->
          <div v-if="itemIconInfo.itemType === 2">
            <NInput v-model:value="itemIconInfo.src" class="mb-[5px] w-full" size="small" type="text" :placeholder="$t('iconItem.inputIconUrlOrUpload')" @input="handleChange" />
            <NUpload
              :custom-request="handleUpload"
              :show-file-list="false"
            >
              <NButton size="small">
                {{ $t('iconItem.selectUpload') }}
              </NButton>
            </NUpload>
          </div>
        </div>
      </div>

      <div class="flex items-center mt-[10px]">
        <div class="w-auto text-slate-500 mr-[10px]">
          {{ $t('common.backgroundColor') }}
        </div>
        <div class="w-[150px] flex items-center mr-[10px]">
          <NColorPicker
            v-model:value="itemIconInfo.backgroundColor"
            size="small"
            :modes="['hex']"
            :swatches="defautSwatchesBackground"
            @complete="handleChange"
            @update-value="handleChange"
          />
        </div>
        <div v-if="itemIconInfo.backgroundColor !== initData.backgroundColor" class="w-auto text-slate-500 mr-[10px] cursor-pointer">
          <NButton quaternary type="info" @click="handleResetBackgroundColor">
            {{ $t('common.reset') }}
          </NButton>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.transparent-grid {
    background-image: linear-gradient(45deg, #fff 25%, transparent 25%, transparent 75%, #fff 75%),
                      linear-gradient(45deg, #fff 25%, transparent 25%, transparent 75%, #fff 75%);
    background-size: 16px 16px;
    background-position: 0 0, 8px 8px;
}
</style>
