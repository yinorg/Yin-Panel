<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { NButton, NCard, NInput, NList, NListItem, NModal, NProgress, NSelect, NSpace, useMessage } from 'naive-ui'
import { addMember, addOIDCGroup, clearSpace, copySpace, createGroup, createSpace, deleteGroup, deleteOIDCGroup, getGroups, getMembers, getOIDCGroups, getPublicConfig, getSpaces, importBookmarks, renameSpace, setPublicConfig, spaceDisplayName, updateGroup, updateMember, type Space, type SpaceMember } from '../../../api/panel/space'
import { useAuthStore } from '../../../store'
import { t } from '../../../locales'

const message = useMessage()
const authStore = useAuthStore()
const spaces = ref<Space[]>([])
const selectedSpaceId = ref<number | null>(null)
const name = ref(''); const groupName = ref(''); const groupParentId = ref<number | null>(null); const editing = ref<{ spaceId: number; id: number } | null>(null)
const loading = ref(false)
const createDialogVisible = ref(false)
const renameDialogVisible = ref(false)
const groupDialogVisible = ref(false)
const memberDialogVisible = ref(false)
const oidcDialogVisible = ref(false)
const groups = ref<Record<number, { id: number; title: string }[]>>({})
const members = ref<SpaceMember[]>([]); const oidcRules = ref<any[]>([])
const rename = ref(''); const memberEmail = ref(''); const memberRole = ref('viewer'); const oidcProvider = ref('authentik'); const oidcGroup = ref(''); const oidcRole = ref('viewer')
const publicEnabled = ref(false); const publicId = ref(''); const publicMode = ref<'direct' | 'code'>('direct'); const publicAccessCode = ref(''); const publicSaving = ref(false)
const publicOrigin = typeof window !== 'undefined' ? window.location.origin : ''
const bookmarkPreview = ref<any[] | null>(null)
const importing = ref(false); const importProgress = ref(0)
const bookmarkFileInput = ref<HTMLInputElement | null>(null)
function exportBookmarks() { if (!selectedSpaceId.value) return; fetch(`/api/spaces/${selectedSpaceId.value}/bookmarks/export`, { headers: { Authorization: `Bearer ${authStore.token}` } }).then(r => r.blob()).then(blob => { const a = document.createElement('a'); a.href = URL.createObjectURL(blob); a.download = `space-${selectedSpaceId.value}-bookmarks.html`; a.click(); URL.revokeObjectURL(a.href) }) }
function previewBookmarks(event: Event) { const file = (event.target as HTMLInputElement).files?.[0]; if (!file) return; file.text().then(text => { const doc = new DOMParser().parseFromString(text, 'text/html'); const groups: any[] = []; doc.querySelectorAll('h3').forEach(h3 => { const group: any = { title: h3.textContent?.trim() || '未命名分组', items: [] }; let node = h3.parentElement?.nextElementSibling; while (node) { node.querySelectorAll?.('a').forEach((a: HTMLAnchorElement) => group.items.push({ title: a.textContent?.trim() || a.href, url: a.href })); node = node.nextElementSibling } groups.push(group) }); bookmarkPreview.value = groups }) }
function confirmImport() { if (!selectedSpaceId.value || !bookmarkPreview.value || importing.value) return; const count = bookmarkPreview.value.reduce((sum, group) => sum + group.items.length, 0); if (count > 2000 && !window.confirm(t('spaceManage.importConfirm', { count }))) return; if (count > 2000 && !window.confirm(t('spaceManage.importConfirmAgain'))) return; importing.value = true; importProgress.value = 15; const timer = window.setInterval(() => { if (importProgress.value < 90) importProgress.value += 5 }, 500); importBookmarks(selectedSpaceId.value, { groups: bookmarkPreview.value }).then(({ code }) => { if (code === 0) { importProgress.value = 100; message.success(t('spaceManage.importSuccess')); bookmarkPreview.value = null; loadGroups(selectedSpaceId.value!) } }).finally(() => { window.clearInterval(timer); importing.value = false; importProgress.value = 0 }) }
function clearCurrentSpace() { if (!selectedSpaceId.value || !window.confirm(t('spaceManage.clearConfirm'))) return; clearSpace(selectedSpaceId.value).then(({ code }) => { if (code === 0) { message.success(t('spaceManage.clearSuccess')); loadGroups(selectedSpaceId.value!) } }) }
function load() { getSpaces<{ code: number; data: Space[] }>().then(({ data }) => { spaces.value = data || []; if (spaces.value.length && !selectedSpaceId.value) selectedSpaceId.value = spaces.value[0].id; spaces.value.forEach(space => loadGroups(space.id)); if (selectedSpaceId.value) loadDetails(selectedSpaceId.value) }) }
function loadGroups(spaceId: number) { getGroups<{ code: number; data: { id: number; title: string }[] }>(spaceId).then(({ data }) => { groups.value[spaceId] = data || [] }) }
function loadDetails(spaceId: number) { getMembers<{ code: number; data: SpaceMember[] }>(spaceId).then(({ data }) => { members.value = data || [] }); getOIDCGroups<{ code: number; data: any[] }>(spaceId).then(({ data }) => { oidcRules.value = data || [] }); getPublicConfig<{ code: number; data: any }>(spaceId).then(({ data }) => { publicEnabled.value = !!data?.enabled; publicId.value = data?.publicId || ''; publicMode.value = data?.mode === 'code' ? 'code' : 'direct'; publicAccessCode.value = '' }) }
function savePublic() { if (!selectedSpaceId.value || (publicEnabled.value && !/^[a-z][a-z0-9-]{4,28}[a-z0-9]$/.test(publicId.value))) { message.error(t('spaceManage.publicIdInvalid')); return }; if (publicMode.value === 'code' && publicEnabled.value && publicAccessCode.value && (publicAccessCode.value.length < 4 || publicAccessCode.value.length > 12)) { message.error(t('spaceManage.accessCodeInvalid')); return }; publicSaving.value = true; setPublicConfig(selectedSpaceId.value, { enabled: publicEnabled.value, publicId: publicId.value, mode: publicMode.value, accessCode: publicAccessCode.value }).then(({ code }) => { if (code === 0) message.success(t('spaceManage.publicSaved')) }).finally(() => { publicSaving.value = false }) }
function saveGroup(spaceId: number) {
  const title = groupName.value.trim(); if (!title) return false
  const req = editing.value ? updateGroup(spaceId, editing.value.id, title, '', groupParentId.value) : createGroup(spaceId, title, '', groupParentId.value)
  return req.then(({ code }) => {
    if (code !== 0) return false
    groupName.value = ''
    groupParentId.value = null
    editing.value = null
    groupDialogVisible.value = false
    loadGroups(spaceId)
    return true
  })
}
function openAddGroup(spaceId: number) { editing.value = null; groupName.value = ''; groupParentId.value = null; groupDialogVisible.value = true }
function editGroup(spaceId: number, group: { id: number; title: string; parentId?: number | null }) { editing.value = { spaceId, id: group.id }; groupName.value = group.title; groupParentId.value = group.parentId || null; groupDialogVisible.value = true }
function removeGroup(spaceId: number, id: number) { if (!window.confirm(t('spaceManage.groupDeleteConfirm'))) return; deleteGroup(spaceId, id).then(({ code }) => { if (code === 0) loadGroups(spaceId) }) }
function openCreateDialog() {
  name.value = ''
  createDialogVisible.value = true
}
async function create() {
  if (!name.value.trim() || loading.value) return false
  loading.value = true
  try {
    const { code } = await createSpace<{ code: number }>(name.value.trim())
    if (code !== 0) return false
    message.success('空间创建成功')
    name.value = ''
    createDialogVisible.value = false
    load()
    return true
  }
  finally {
    loading.value = false
  }
}
function selected() { return spaces.value.find(space => space.id === selectedSpaceId.value) }
function openRenameDialog() {
  rename.value = selected()?.name || ''
  renameDialogVisible.value = true
}
async function renameCurrent() {
  const value = rename.value.trim()
  if (!value || !selectedSpaceId.value) return false
  const { code } = await renameSpace(selectedSpaceId.value, value)
  if (code !== 0) return false
  message.success('空间名称已更新')
  renameDialogVisible.value = false
  load()
  return true
}
function copyCurrent() { if (selectedSpaceId.value) copySpace<{ code: number }>(selectedSpaceId.value).then(({ code }) => { if (code === 0) { message.success('空间已复制'); load() } }) }
async function addCurrentMember() {
  const email = memberEmail.value.trim()
  if (!selectedSpaceId.value || !email) return false
  const { code } = await addMember(selectedSpaceId.value, email, memberRole.value)
  if (code !== 0) return false
  memberEmail.value = ''
  memberRole.value = 'viewer'
  memberDialogVisible.value = false
  loadDetails(selectedSpaceId.value)
  return true
}
function openAddMember() { memberEmail.value = ''; memberRole.value = 'viewer'; memberDialogVisible.value = true }
function changeMember(member: SpaceMember, role: string) { if (selectedSpaceId.value) updateMember(selectedSpaceId.value, member.userId, role).then(() => loadDetails(selectedSpaceId.value!)) }
async function addRule() {
  if (!selectedSpaceId.value || !oidcGroup.value.trim()) return false
  const { code } = await addOIDCGroup(selectedSpaceId.value, oidcProvider.value, oidcGroup.value.trim(), oidcRole.value)
  if (code !== 0) return false
  oidcGroup.value = ''
  oidcProvider.value = 'authentik'
  oidcRole.value = 'viewer'
  oidcDialogVisible.value = false
  loadDetails(selectedSpaceId.value)
  return true
}
function openAddRule() { oidcProvider.value = 'authentik'; oidcGroup.value = ''; oidcRole.value = 'viewer'; oidcDialogVisible.value = true }
function removeRule(id: number) { if (selectedSpaceId.value) deleteOIDCGroup(selectedSpaceId.value, id).then(() => loadDetails(selectedSpaceId.value!)) }
onMounted(load)
</script>

<template>
  <div class="p-4">
    <NCard :title="$t('spaceManage.title')">
      <NSpace vertical>
        <NSpace>
          <NButton type="primary" @click="openCreateDialog">{{ $t('spaceManage.createSpace') }}</NButton>
        </NSpace>
        <NModal v-model:show="createDialogVisible" preset="dialog" :title="$t('spaceManage.createSpace')" :positive-text="$t('common.confirm')" :negative-text="$t('common.cancel')" :loading="loading" @positive-click="create">
          <NInput v-model:value="name" :placeholder="$t('spaceManage.newSpaceName')" maxlength="100" @keyup.enter="create" />
        </NModal>
        <NSelect v-model:value="selectedSpaceId" :options="spaces.map(space => ({ label: `${spaceDisplayName(space, spaces, authStore.userInfo?.id, true)} (${space.type === 'shared' || space.type === 'team' ? $t('spaceManage.shared') : $t('spaceManage.personal')})`, value: space.id }))" @update:value="(id) => loadDetails(Number(id))" />
        <NSpace v-if="selectedSpaceId"><NButton @click="openRenameDialog">{{ $t('spaceManage.rename') }}</NButton><NButton @click="copyCurrent">{{ $t('spaceManage.copy') }}</NButton></NSpace>
        <NModal v-model:show="renameDialogVisible" preset="dialog" :title="$t('spaceManage.rename')" :positive-text="$t('common.confirm')" :negative-text="$t('common.cancel')" @positive-click="renameCurrent">
          <NInput v-model:value="rename" :placeholder="$t('spaceManage.spaceName')" maxlength="100" @keyup.enter="renameCurrent" />
        </NModal>
        <NCard v-if="selectedSpaceId" :title="$t('spaceManage.publicAccess')">
          <NSpace vertical>
            <label><input v-model="publicEnabled" type="checkbox"> {{ $t('spaceManage.enablePublicAccess') }}</label>
            <NInput v-model:value="publicId" :placeholder="$t('spaceManage.publicIdPlaceholder')" maxlength="30" />
            <NSelect v-model:value="publicMode" :options="[{ label: $t('spaceManage.directAccess'), value: 'direct' }, { label: $t('spaceManage.codeAccess'), value: 'code' }]" />
            <NInput v-if="publicMode === 'code'" v-model:value="publicAccessCode" type="password" show-password-on="click" :placeholder="$t('spaceManage.accessCodePlaceholder')" maxlength="12" />
            <span v-if="publicEnabled && publicId" class="text-gray-500">访问地址：{{ `${publicOrigin}/${publicId}` }}</span>
            <NButton type="primary" :loading="publicSaving" @click="savePublic">{{ $t('spaceManage.savePublic') }}</NButton>
          </NSpace>
        </NCard>
        <NCard v-if="selectedSpaceId" :title="$t('spaceManage.bookmarkTransfer')">
          <NSpace><NButton @click="exportBookmarks">{{ $t('spaceManage.exportBookmarks') }}</NButton><NButton @click="bookmarkFileInput?.click()">{{ $t('spaceManage.chooseFile') }}</NButton><input ref="bookmarkFileInput" class="hidden" type="file" accept=".html,text/html" @change="previewBookmarks"></NSpace>
          <NList v-if="bookmarkPreview" class="mt-2"><NListItem v-for="group in bookmarkPreview" :key="group.title"><span>{{ group.title }}（{{ group.items.length }}项）</span></NListItem><NProgress v-if="importing" type="line" :percentage="importProgress" processing /><NButton type="primary" :loading="importing" @click="confirmImport">确认导入</NButton></NList>
        </NCard>
        <NButton v-if="selectedSpaceId" type="error" secondary @click="clearCurrentSpace">{{ $t('spaceManage.clearSpace') }}</NButton>
        <NList v-if="selectedSpaceId" bordered>
          <NListItem>
            <div class="w-full">
              <div class="flex items-center gap-2"><span>{{ $t('spaceManage.groups') }}</span><NButton size="tiny" type="primary" @click="openAddGroup(selectedSpaceId!)">{{ $t('spaceManage.addGroup') }}</NButton><span v-if="!groups[selectedSpaceId]?.length" class="text-gray-500">{{ $t('common.noData') }}</span></div>
              <NSpace vertical size="small" class="mt-1"><NButton v-for="group in groups[selectedSpaceId]" :key="group.id" size="tiny" secondary @click="editGroup(selectedSpaceId!, group)">{{ group.title }}{{ group.parentId ? ' (子分组)' : '' }}</NButton></NSpace>
            </div>
          </NListItem>
        </NList>
        <NModal v-if="selectedSpaceId" v-model:show="groupDialogVisible" preset="dialog" :title="editing ? $t('common.edit') : $t('spaceManage.addGroup')" :positive-text="$t('common.confirm')" :negative-text="$t('common.cancel')" @positive-click="saveGroup(selectedSpaceId!)">
          <NSpace vertical>
            <NInput v-model:value="groupName" :placeholder="$t('spaceManage.groupName')" maxlength="100" @keyup.enter="saveGroup(selectedSpaceId!)" />
            <NSelect v-model:value="groupParentId" clearable :placeholder="$t('spaceManage.parentGroupOptional')" :options="(groups[selectedSpaceId] || []).filter(group => group.id !== editing?.id).map(group => ({ label: group.title, value: group.id }))" />
          </NSpace>
        </NModal>
        <NCard v-if="selectedSpaceId" :title="$t('spaceManage.members')">
          <template #header-extra><NButton size="tiny" type="primary" @click="openAddMember">{{ $t('spaceManage.addMember') }}</NButton></template>
          <NList><NListItem v-for="member in members" :key="member.id"><NSpace justify="space-between" class="w-full"><span>{{ member.email || $t('spaceManage.emailMissing') }} ({{ member.source || 'manual' }})</span><NSelect :value="member.role" :options="[{ label: $t('spaceManage.admin'), value: 'admin' }, { label: $t('spaceManage.editor'), value: 'editor' }, { label: $t('spaceManage.viewer'), value: 'viewer' }]" @update:value="role => changeMember(member, role)" /></NSpace></NListItem></NList>
        </NCard>
        <NModal v-if="selectedSpaceId" v-model:show="memberDialogVisible" preset="dialog" :title="$t('spaceManage.addMember')" :positive-text="$t('common.confirm')" :negative-text="$t('common.cancel')" @positive-click="addCurrentMember">
          <NSpace vertical>
            <NInput v-model:value="memberEmail" :placeholder="$t('spaceManage.userEmail')" @keyup.enter="addCurrentMember" />
            <NSelect v-model:value="memberRole" :options="[{ label: $t('spaceManage.editor'), value: 'editor' }, { label: $t('spaceManage.viewer'), value: 'viewer' }]" />
          </NSpace>
        </NModal>
        <NCard v-if="selectedSpaceId" :title="$t('spaceManage.oidcTitle')">
          <template #header-extra><NButton size="tiny" type="primary" @click="openAddRule">{{ $t('spaceManage.addRule') }}</NButton></template>
          <NList><NListItem v-for="rule in oidcRules" :key="rule.id"><NSpace justify="space-between" class="w-full"><span>{{ rule.provider }} / {{ rule.groupName }} ({{ rule.role }})</span><NButton size="small" @click="removeRule(rule.id)">{{ $t('common.delete') }}</NButton></NSpace></NListItem></NList>
        </NCard>
        <NModal v-if="selectedSpaceId" v-model:show="oidcDialogVisible" preset="dialog" :title="$t('spaceManage.addRule')" :positive-text="$t('common.confirm')" :negative-text="$t('common.cancel')" @positive-click="addRule">
          <NSpace vertical>
            <NInput v-model:value="oidcProvider" placeholder="Provider" />
            <NInput v-model:value="oidcGroup" :placeholder="$t('spaceManage.groupName')" @keyup.enter="addRule" />
            <NSelect v-model:value="oidcRole" :options="[{ label: $t('spaceManage.editor'), value: 'editor' }, { label: $t('spaceManage.viewer'), value: 'viewer' }]" />
          </NSpace>
        </NModal>
      </NSpace>
    </NCard>
  </div>
</template>
