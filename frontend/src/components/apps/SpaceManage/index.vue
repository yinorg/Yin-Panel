<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { NButton, NCard, NInput, NList, NListItem, NSelect, NSpace, useMessage } from 'naive-ui'
import { createGroup, createTeam, deleteGroup, getGroups, getSpaces, updateGroup, type Space } from '../../../api/panel/space'

const message = useMessage()
const spaces = ref<Space[]>([])
const selectedSpaceId = ref<number | null>(null)
const name = ref(''); const groupName = ref(''); const editing = ref<{ spaceId: number; id: number } | null>(null)
const loading = ref(false)
const groups = ref<Record<number, { id: number; title: string }[]>>({})
function load() { getSpaces<{ code: number; data: Space[] }>().then(({ data }) => { spaces.value = data || []; if (spaces.value.length && !selectedSpaceId.value) selectedSpaceId.value = spaces.value[0].id; spaces.value.forEach(space => loadGroups(space.id)) }) }
function loadGroups(spaceId: number) { getGroups<{ code: number; data: { id: number; title: string }[] }>(spaceId).then(({ data }) => { groups.value[spaceId] = data || [] }) }
function saveGroup(spaceId: number) {
  const title = groupName.value.trim(); if (!title) return
  const req = editing.value ? updateGroup(spaceId, editing.value.id, title) : createGroup(spaceId, title)
  req.then(({ code }) => { if (code === 0) { groupName.value = ''; editing.value = null; loadGroups(spaceId) } })
}
function removeGroup(spaceId: number, id: number) { deleteGroup(spaceId, id).then(({ code }) => { if (code === 0) loadGroups(spaceId) }) }
function create() {
  if (!name.value.trim() || loading.value) return
  loading.value = true
  createTeam<{ code: number }>(name.value.trim()).then(({ code }) => { if (code === 0) { message.success('团队创建成功'); name.value = ''; load() } }).finally(() => { loading.value = false })
}
onMounted(load)
</script>

<template>
  <div class="p-4">
    <NCard title="空间管理">
      <NSpace vertical>
        <NSpace>
          <NInput v-model:value="name" placeholder="团队名称" maxlength="100" @keyup.enter="create" />
          <NButton type="primary" :loading="loading" @click="create">创建团队</NButton>
        </NSpace>
        <NSelect v-model:value="selectedSpaceId" :options="spaces.map(space => ({ label: `${space.name} (${space.type === 'shared' ? '共享' : '个人'})`, value: space.id }))" />
        <NList v-if="selectedSpaceId" bordered>
          <NListItem>
            <div class="w-full">
              <div class="flex items-center gap-2"><span>当前空间分组</span><span v-if="!groups[selectedSpaceId]?.length" class="text-gray-500">暂无</span></div>
              <NSpace size="small" class="mt-1"><NButton v-for="group in groups[selectedSpaceId]" :key="group.id" size="tiny" secondary @click="editing = { spaceId: selectedSpaceId!, id: group.id }; groupName = group.title">{{ group.title }}</NButton></NSpace>
              <NSpace size="small" class="mt-2"><NInput v-model:value="groupName" size="small" placeholder="分组名称" /><NButton size="small" @click="saveGroup(selectedSpaceId!)">{{ editing?.spaceId === selectedSpaceId ? '保存' : '新增分组' }}</NButton><NButton v-if="editing?.spaceId === selectedSpaceId" size="small" type="error" @click="removeGroup(selectedSpaceId!, editing.id)">删除</NButton></NSpace>
            </div>
          </NListItem>
        </NList>
      </NSpace>
    </NCard>
  </div>
</template>
