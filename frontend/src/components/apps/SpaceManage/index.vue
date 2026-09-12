<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { NButton, NCard, NInput, NList, NListItem, NSelect, NSpace, useMessage } from 'naive-ui'
import { addMember, addOIDCGroup, copySpace, createGroup, createTeam, deleteGroup, deleteOIDCGroup, getGroups, getMembers, getOIDCGroups, getPublicConfig, getSpaces, renameSpace, setPublicConfig, spaceDisplayName, updateGroup, updateMember, type Space, type SpaceMember } from '../../../api/panel/space'
import { useAuthStore } from '../../../store'

const message = useMessage()
const authStore = useAuthStore()
const spaces = ref<Space[]>([])
const selectedSpaceId = ref<number | null>(null)
const name = ref(''); const groupName = ref(''); const editing = ref<{ spaceId: number; id: number } | null>(null)
const loading = ref(false)
const groups = ref<Record<number, { id: number; title: string }[]>>({})
const members = ref<SpaceMember[]>([]); const oidcRules = ref<any[]>([])
const rename = ref(''); const memberEmail = ref(''); const memberRole = ref('viewer'); const oidcProvider = ref('authentik'); const oidcGroup = ref(''); const oidcRole = ref('viewer')
const publicEnabled = ref(false); const publicId = ref(''); const publicMode = ref<'direct' | 'code'>('direct'); const publicAccessCode = ref(''); const publicSaving = ref(false)
const publicOrigin = typeof window !== 'undefined' ? window.location.origin : ''
function load() { getSpaces<{ code: number; data: Space[] }>().then(({ data }) => { spaces.value = data || []; if (spaces.value.length && !selectedSpaceId.value) selectedSpaceId.value = spaces.value[0].id; spaces.value.forEach(space => loadGroups(space.id)); if (selectedSpaceId.value) loadDetails(selectedSpaceId.value) }) }
function loadGroups(spaceId: number) { getGroups<{ code: number; data: { id: number; title: string }[] }>(spaceId).then(({ data }) => { groups.value[spaceId] = data || [] }) }
function loadDetails(spaceId: number) { getMembers<{ code: number; data: SpaceMember[] }>(spaceId).then(({ data }) => { members.value = data || [] }); getOIDCGroups<{ code: number; data: any[] }>(spaceId).then(({ data }) => { oidcRules.value = data || [] }); getPublicConfig<{ code: number; data: any }>(spaceId).then(({ data }) => { publicEnabled.value = !!data?.enabled; publicId.value = data?.publicId || ''; publicMode.value = data?.mode === 'code' ? 'code' : 'direct'; publicAccessCode.value = '' }) }
function savePublic() { if (!selectedSpaceId.value || (publicEnabled.value && !/^[a-z][a-z0-9-]{4,28}[a-z0-9]$/.test(publicId.value))) { message.error('FN ID需为6-30位小写字母、数字或连字符'); return }; if (publicMode.value === 'code' && publicEnabled.value && publicAccessCode.value && (publicAccessCode.value.length < 4 || publicAccessCode.value.length > 12)) { message.error('访问码需为4-12个字符'); return }; publicSaving.value = true; setPublicConfig(selectedSpaceId.value, { enabled: publicEnabled.value, publicId: publicId.value, mode: publicMode.value, accessCode: publicAccessCode.value }).then(({ code }) => { if (code === 0) message.success('公开访问配置已保存') }).finally(() => { publicSaving.value = false }) }
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
function selected() { return spaces.value.find(space => space.id === selectedSpaceId.value) }
function renameCurrent() { const value = rename.value.trim(); if (value && selectedSpaceId.value) renameSpace(selectedSpaceId.value, value).then(({ code }) => { if (code === 0) { message.success('空间名称已更新'); load() } }) }
function copyCurrent() { if (selectedSpaceId.value) copySpace<{ code: number }>(selectedSpaceId.value).then(({ code }) => { if (code === 0) { message.success('空间已复制'); load() } }) }
function addCurrentMember() { const email = memberEmail.value.trim(); if (selectedSpaceId.value && email) addMember(selectedSpaceId.value, email, memberRole.value).then(({ code }) => { if (code === 0) { memberEmail.value = ''; loadDetails(selectedSpaceId.value!) } }) }
function changeMember(member: SpaceMember, role: string) { if (selectedSpaceId.value) updateMember(selectedSpaceId.value, member.userId, role).then(() => loadDetails(selectedSpaceId.value!)) }
function addRule() { if (selectedSpaceId.value && oidcGroup.value.trim()) addOIDCGroup(selectedSpaceId.value, oidcProvider.value, oidcGroup.value.trim(), oidcRole.value).then(({ code }) => { if (code === 0) { oidcGroup.value = ''; loadDetails(selectedSpaceId.value!) } }) }
function removeRule(id: number) { if (selectedSpaceId.value) deleteOIDCGroup(selectedSpaceId.value, id).then(() => loadDetails(selectedSpaceId.value!)) }
onMounted(load)
</script>

<template>
  <div class="p-4">
    <NCard title="空间管理">
      <NSpace vertical>
        <NSpace>
          <NInput v-model:value="name" placeholder="新空间名称" maxlength="100" @keyup.enter="create" />
          <NButton type="primary" :loading="loading" @click="create">创建新空间</NButton>
        </NSpace>
        <NSelect v-model:value="selectedSpaceId" :options="spaces.map(space => ({ label: `${spaceDisplayName(space, spaces, authStore.userInfo?.id, true)} (${space.type === 'shared' || space.type === 'team' ? '共享' : '个人'})`, value: space.id }))" @update:value="(id) => loadDetails(Number(id))" />
        <NSpace v-if="selectedSpaceId"><NInput v-model:value="rename" :placeholder="selected()?.name || '空间名称'" /><NButton @click="renameCurrent">重命名</NButton><NButton @click="copyCurrent">复制空间</NButton></NSpace>
        <NCard v-if="selectedSpaceId" title="公开访问">
          <NSpace vertical>
            <label><input v-model="publicEnabled" type="checkbox"> 开启公开访问</label>
            <NInput v-model:value="publicId" placeholder="FN ID，例如 my-panel" maxlength="30" />
            <NSelect v-model:value="publicMode" :options="[{ label: '链接直接访问', value: 'direct' }, { label: '访问码验证', value: 'code' }]" />
            <NInput v-if="publicMode === 'code'" v-model:value="publicAccessCode" type="password" show-password-on="click" placeholder="访问码（4-12个字符，留空保持不变）" maxlength="12" />
            <span v-if="publicEnabled && publicId" class="text-gray-500">访问地址：{{ `${publicOrigin}/${publicId}` }}</span>
            <NButton type="primary" :loading="publicSaving" @click="savePublic">保存公开访问配置</NButton>
          </NSpace>
        </NCard>
        <NList v-if="selectedSpaceId" bordered>
          <NListItem>
            <div class="w-full">
              <div class="flex items-center gap-2"><span>当前空间分组</span><span v-if="!groups[selectedSpaceId]?.length" class="text-gray-500">暂无</span></div>
              <NSpace size="small" class="mt-1"><NButton v-for="group in groups[selectedSpaceId]" :key="group.id" size="tiny" secondary @click="editing = { spaceId: selectedSpaceId!, id: group.id }; groupName = group.title">{{ group.title }}</NButton></NSpace>
              <NSpace size="small" class="mt-2"><NInput v-model:value="groupName" size="small" placeholder="分组名称" /><NButton size="small" @click="saveGroup(selectedSpaceId!)">{{ editing?.spaceId === selectedSpaceId ? '保存' : '新增分组' }}</NButton><NButton v-if="editing?.spaceId === selectedSpaceId" size="small" type="error" @click="removeGroup(selectedSpaceId!, editing.id)">删除</NButton></NSpace>
            </div>
          </NListItem>
        </NList>
        <NCard v-if="selectedSpaceId" title="成员管理">
          <NSpace><NInput v-model:value="memberEmail" placeholder="用户邮箱" /><NSelect v-model:value="memberRole" :options="[{ label: '编辑者', value: 'editor' }, { label: '查看者', value: 'viewer' }]" /><NButton @click="addCurrentMember">添加成员</NButton></NSpace>
          <NList><NListItem v-for="member in members" :key="member.id"><NSpace justify="space-between" class="w-full"><span>{{ member.email || '邮箱未设置' }} ({{ member.source || 'manual' }})</span><NSelect :value="member.role" :options="[{ label: '管理员', value: 'admin' }, { label: '编辑者', value: 'editor' }, { label: '查看者', value: 'viewer' }]" @update:value="role => changeMember(member, role)" /></NSpace></NListItem></NList>
        </NCard>
        <NCard v-if="selectedSpaceId" title="OIDC 分组授权">
          <NSpace><NInput v-model:value="oidcProvider" placeholder="Provider" /><NInput v-model:value="oidcGroup" placeholder="分组名称" /><NSelect v-model:value="oidcRole" :options="[{ label: '编辑者', value: 'editor' }, { label: '查看者', value: 'viewer' }]" /><NButton @click="addRule">添加规则</NButton></NSpace>
          <NList><NListItem v-for="rule in oidcRules" :key="rule.id"><NSpace justify="space-between" class="w-full"><span>{{ rule.provider }} / {{ rule.groupName }} ({{ rule.role }})</span><NButton size="small" @click="removeRule(rule.id)">删除</NButton></NSpace></NListItem></NList>
        </NCard>
      </NSpace>
    </NCard>
  </div>
</template>
