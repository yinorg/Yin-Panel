<script setup lang="ts">
import { NButton, NCard, NInput, NList, NListItem, NModal, NProgress, NSelect, NSpace, useMessage } from 'naive-ui'
import { computed, onMounted, ref } from 'vue'
import { addMember, addOIDCGroup, clearSpace, copySpace, createGroup, createSpace, deleteOIDCGroup, deleteSpace, getGroups, getMembers, getOIDCGroups, getPublicConfig, getSearchConfig, getSpaces, removeMember, renameSpace, resolveSpace, setPublicConfig, setSearchConfig, spaceOptions, updateGroup, updateMember, type Space, type SpaceMember, type SpaceSearchConfig } from '../../../api/panel/space'
import { useAuthStore } from '../../../store'
import { t } from '../../../locales'
import { clearSpaceCache, readSpaceCache, writeSpaceCache } from '@/utils/spaceCache'
import { fetchIconsFromExtension, getIconByUrl, iconFileFromBytes, isIconExtensionAvailable } from '@/utils/itemIcon'
import { importBookmarksBatch } from '@/api/panel/space'
import { getBookmarkTree } from '@/api/panel/space'
import { buildBookmarkHtml, parseBookmarkHtml, type BookmarkLink, type BookmarkNode } from '@/utils/bookmarks'
import { getInstalledThemes, clearSpaceTheme, getSpaceTheme, setSpaceTheme } from '@/api/theme'
import { searchEngineList } from '@/components/deskModule/SearchBox/engines'
import { useConfirmAction } from '@/hooks/useConfirmAction'

const message = useMessage()
const { confirmAction } = useConfirmAction()
const authStore = useAuthStore()
const emit = defineEmits<{
  (e: 'spaces-changed'): void
}>()
const spaces = ref<Space[]>([])
const selectedSpaceId = ref<number | null>(null)
const name = ref(''); const groupName = ref(''); const groupParentId = ref<number | null>(null); const editing = ref<{ spaceId: number; id: number } | null>(null)
const loading = ref(false)
const createDialogVisible = ref(false)
const renameDialogVisible = ref(false)
const groupDialogVisible = ref(false)
const memberDialogVisible = ref(false)
const oidcDialogVisible = ref(false)
const groups = ref<Record<number, { id: number; title: string; parentId?: number | null }[]>>({})
const members = ref<SpaceMember[]>([]); const oidcRules = ref<any[]>([])
const rename = ref(''); const memberEmail = ref(''); const memberRole = ref('viewer'); const oidcProvider = ref('authentik'); const oidcGroup = ref(''); const oidcRole = ref('viewer')
const publicEnabled = ref(false); const publicId = ref(''); const publicMode = ref<'direct' | 'code'>('direct'); const publicAccessCode = ref(''); const publicSaving = ref(false)
const publicOrigin = typeof window !== 'undefined' ? window.location.origin : ''
const bookmarkPreview = ref<BookmarkNode[] | null>(null)
const importing = ref(false); const importProgress = ref(0)
const bookmarkFileInput = ref<HTMLInputElement | null>(null)
// The space's own theme and its default search engine are properties of the
// space, so they live here rather than in the personal appearance page.
const spaceThemeSelection = ref('')
const spaceThemeSaving = ref(false)
const spaceThemeOptions = ref<{ label: string; value: string }[]>([])
const selectedSearchEngineUrl = ref(searchEngineList[0].url)
const searchConfigSaving = ref(false)
function invalidateSpaceCache(spaceId: number) { clearSpaceCache(spaceId, authStore.userInfo?.id) }
function createTextIcon(title: string): Panel.ItemIcon {
  const chinese = title.match(/[\u3400-\u9fff]/g)?.join('').slice(0, 5) || ''
  const english = title.match(/[A-Za-z]/g)?.join('').slice(0, 8) || ''
  return { itemType: 1, text: chinese || english || title.trim().slice(0, 5) || '?', backgroundColor: '#2a2a2a6b' }
}
/** Every link in the tree, at any depth: the folder level is not where the links stop. */
function flattenBookmarkLinks(nodes: BookmarkNode[]): BookmarkLink[] {
  return nodes.flatMap(node => [...node.items, ...flattenBookmarkLinks(node.children)])
}
async function enrichBookmarkIcons(groups: BookmarkNode[], onProgress: (completed: number) => void) {
  const items = flattenBookmarkLinks(groups)
  let nextIndex = 0; const uploads = new Map<string, File>()
  let completed = 0
  const extensionResults = await (await isIconExtensionAvailable() ? fetchIconsFromExtension(items.map(item => item.url)) : Promise.resolve(null))
  const extensionIcons = new Map((extensionResults || []).map(item => [item.url, item]))
  const worker = async () => {
    while (nextIndex < items.length) {
      const item = items[nextIndex++]
      try {
        const fetched = await iconFileFromBytes(extensionIcons.get(item.url)!)
        if (fetched) { const key = `${fetched.hash}${fetched.ext}`; uploads.set(key, fetched.file); item.uploadKey = key; item.icon = { itemType: 2, fileName: fetched.file.name } }
        else item.icon = await getIconByUrl(item.url) || createTextIcon(item.title)
      } catch {
        item.icon = createTextIcon(item.title)
      } finally {
        onProgress(++completed)
      }
    }
  }
  await Promise.all(Array.from({ length: Math.min(1000, items.length) }, () => worker()))
  return uploads
}
/**
 * The picker writes the file, not the server: the format is the browser's, and
 * the same module reads it back on import, so one place owns both directions.
 */
async function exportBookmarks() {
  if (!selectedSpaceId.value) return
  const { code, data } = await getBookmarkTree<{ groups: BookmarkNode[] }>(selectedSpaceId.value)
  if (code !== 0 || !data) return
  const blob = new Blob([buildBookmarkHtml(data.groups)], { type: 'text/html' })
  const a = document.createElement('a')
  a.href = URL.createObjectURL(blob)
  a.download = `space-${selectedSpaceId.value}-bookmarks.html`
  a.click()
  URL.revokeObjectURL(a.href)
}
async function previewBookmarks(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0]
  if (!file) return
  bookmarkPreview.value = parseBookmarkHtml(await file.text(), t('spaceManage.untitledGroup'))
}
/** The preview is the tree flattened with a depth, so nesting is visible at a glance. */
const bookmarkPreviewRows = computed(() => {
  const rows: { title: string; count: number; depth: number }[] = []
  const walk = (nodes: BookmarkNode[], depth: number) => {
    for (const node of nodes) {
      rows.push({ title: node.title, count: node.items.length, depth })
      walk(node.children, depth + 1)
    }
  }
  walk(bookmarkPreview.value || [], 0)
  return rows
})
async function confirmImport() {
  if (!selectedSpaceId.value || !bookmarkPreview.value || importing.value) return
  const count = flattenBookmarkLinks(bookmarkPreview.value).length
  if (count > 2000 && !(await confirmAction({ content: t('spaceManage.importConfirm', { count }) }))) return
  if (count > 2000 && !(await confirmAction({ content: t('spaceManage.importConfirmAgain') }))) return
  importing.value = true
  importProgress.value = 5
  try {
    importProgress.value = count ? 10 : 80
    const uploads = await enrichBookmarkIcons(bookmarkPreview.value, completed => {
      importProgress.value = 10 + Math.round(completed / Math.max(count, 1) * 70)
    })
    importProgress.value = 85
    const form = new FormData(); form.append('bookmarks', JSON.stringify({ groups: bookmarkPreview.value }))
    uploads.forEach((file, key) => form.append(key, file, file.name))
    const { code } = await importBookmarksBatch(selectedSpaceId.value, form)
    if (code === 0) {
      invalidateSpaceCache(selectedSpaceId.value)
      importProgress.value = 100
      message.success(t('spaceManage.importSuccess'))
      bookmarkPreview.value = null
      loadGroups(selectedSpaceId.value)
      emit('spaces-changed')
    }
  } finally {
    importing.value = false
    importProgress.value = 0
  }
}
async function clearCurrentSpace() {
  if (!selectedSpaceId.value) return
  if (!(await confirmAction({ content: t('spaceManage.clearConfirm'), positiveText: t('spaceManage.clearSpace'), variant: 'error' }))) return
  const { code } = await clearSpace(selectedSpaceId.value)
  if (code !== 0) return
  invalidateSpaceCache(selectedSpaceId.value)
  message.success(t('spaceManage.clearSuccess'))
  loadGroups(selectedSpaceId.value)
}
function load() { getSpaces<Space[]>().then(({ data }) => { spaces.value = data || []; if (spaces.value.length && !selectedSpaceId.value) selectedSpaceId.value = spaces.value[0].id; spaces.value.forEach(space => loadGroups(space.id)); if (selectedSpaceId.value) { loadGroups(selectedSpaceId.value); loadDetails(selectedSpaceId.value) } }) }
function loadGroups(spaceId: number) { getGroups<{ id: number; title: string; parentId?: number | null }[]>(spaceId).then(({ data }) => { groups.value[spaceId] = data || [] }) }
function loadDetails(spaceId: number) { getMembers<SpaceMember[]>(spaceId).then(({ data }) => { members.value = data || [] }); getOIDCGroups<any[]>(spaceId).then(({ data }) => { oidcRules.value = data || [] }); getPublicConfig<any>(spaceId).then(({ data }) => { publicEnabled.value = !!data?.enabled; publicId.value = data?.publicId || ''; publicMode.value = data?.mode === 'code' ? 'code' : 'direct'; publicAccessCode.value = '' }); loadSpaceTheme(spaceId); loadSearchConfig(spaceId) }
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
    invalidateSpaceCache(spaceId)
    loadGroups(spaceId)
    return true
  })
}
function openAddGroup(spaceId: number) { editing.value = null; groupName.value = ''; groupParentId.value = null; groupDialogVisible.value = true }
function editGroup(spaceId: number, group: { id: number; title: string; parentId?: number | null }) { editing.value = { spaceId, id: group.id }; groupName.value = group.title; groupParentId.value = group.parentId || null; groupDialogVisible.value = true }
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
    message.success(t('spaceManage.createSuccess'))
    name.value = ''
    createDialogVisible.value = false
    load()
    emit('spaces-changed')
    return true
  }
  finally {
    loading.value = false
  }
}
function selected() { return selectedSpaceId.value ? resolveSpace(spaces.value, selectedSpaceId.value) : undefined }
function selectSpace(spaceId: number) { selectedSpaceId.value = spaceId; loadGroups(spaceId); loadDetails(spaceId) }
function openRenameDialog() {
  const space = selected()
  const baseSpace = space?.side === 'yang' ? spaces.value.find(item => item.pairedSpaceId === space.id) : space
  rename.value = baseSpace?.name || space?.name || ''
  renameDialogVisible.value = true
}
async function renameCurrent() {
  const value = rename.value.trim()
  if (!value || !selectedSpaceId.value) return false
  const space = selected()
  const baseSpace = space?.side === 'yang' ? spaces.value.find(item => item.pairedSpaceId === space.id) : space
  const { code } = await renameSpace(baseSpace?.id || selectedSpaceId.value, value)
  if (code !== 0) return false
  message.success(t('spaceManage.renameSuccess'))
  renameDialogVisible.value = false
  load()
  emit('spaces-changed')
  return true
}
function copyCurrent() { if (selectedSpaceId.value) copySpace<{ code: number }>(selectedSpaceId.value).then(({ code }) => { if (code === 0) { message.success(t('spaceManage.copySuccess')); load(); emit('spaces-changed') } }) }
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
// The owner always resolves to the admin role on the Core side, so leaving the
// space is the only way to give it up, and that deletes the space. A personal
// space is the user's own default panel and can never be left or deleted.
function isSpaceOwner(member: SpaceMember) { return member.userId === selected()?.ownerUserId }
function canLeaveSpace() { return selected()?.type !== 'personal' }
/** The nickname first, because the email alone reads as an account id; the email
 *  stays visible as the identity the membership is keyed by. */
function memberLabel(member: SpaceMember) {
  const name = member.name?.trim()
  const email = member.email?.trim() || t('spaceManage.emailMissing')
  return name && name !== email ? `${name} · ${email}` : email
}
async function removeCurrentMember(member: SpaceMember) {
  if (!selectedSpaceId.value || isSpaceOwner(member)) return
  if (!(await confirmAction({ content: t('spaceManage.removeMemberConfirm'), positiveText: t('spaceManage.removeMember') }))) return
  const { code } = await removeMember(selectedSpaceId.value, member.userId)
  if (code !== 0) return
  message.success(t('spaceManage.removeMemberSuccess'))
  loadDetails(selectedSpaceId.value)
}
async function leaveCurrentSpace() {
  if (!selectedSpaceId.value || !canLeaveSpace()) return
  if (!(await confirmAction({ content: t('spaceManage.leaveSpaceConfirm'), positiveText: t('spaceManage.leaveSpace'), variant: 'error' }))) return
  const { code } = await deleteSpace(selectedSpaceId.value)
  if (code !== 0) return
  message.success(t('spaceManage.leaveSpaceSuccess'))
  selectedSpaceId.value = null
  load()
  emit('spaces-changed')
}
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
const editableSpace = computed(() => !!selected()?.canEdit)
async function loadSpaceThemeOptions() {
  if (spaceThemeOptions.value.length) return
  try {
    const { code, data } = await getInstalledThemes()
    if (code === 0)
      spaceThemeOptions.value = [{ label: t('themeScope.followSystem'), value: '' }, ...data.map(item => ({ label: item.name, value: item.id }))]
  }
  catch {
    // The space keeps following the system default when the list is unavailable.
  }
}
async function loadSpaceTheme(spaceId: number) {
  if (!spaces.value.find(space => space.id === spaceId)?.canEdit) return
  await loadSpaceThemeOptions()
  const result = await getSpaceTheme(spaceId)
  if (result.code === 0)
    spaceThemeSelection.value = result.data.theme?.packageId || ''
}
async function saveSpaceTheme() {
  if (!selectedSpaceId.value) return
  spaceThemeSaving.value = true
  try {
    const result = spaceThemeSelection.value
      ? await setSpaceTheme(selectedSpaceId.value, { packageId: spaceThemeSelection.value })
      : await clearSpaceTheme(selectedSpaceId.value)
    if (result.code === 0)
      message.success(t('themeScope.saved'))
    else
      message.error(result.msg)
  }
  finally {
    spaceThemeSaving.value = false
  }
}
function applySearchConfig(config?: SpaceSearchConfig | null) {
  selectedSearchEngineUrl.value = config?.currentSearchEngine?.url || searchEngineList[0].url
}
async function loadSearchConfig(spaceId: number) {
  const cached = readSpaceCache(spaceId, authStore.userInfo?.id)
  if (cached?.searchConfig?.currentSearchEngine?.url) {
    applySearchConfig(cached.searchConfig)
    return
  }
  const { data } = await getSearchConfig<SpaceSearchConfig>(spaceId)
  if (data) {
    applySearchConfig(data)
    const nextCache = readSpaceCache(spaceId, authStore.userInfo?.id)
    if (nextCache) {
      nextCache.searchConfig = data || { currentSearchEngine: searchEngineList[0] }
      writeSpaceCache(spaceId, nextCache, authStore.userInfo?.id)
    }
  }
}
async function saveDefaultSearchEngine() {
  if (!selectedSpaceId.value || searchConfigSaving.value)
    return
  const engine = searchEngineList.find(item => item.url === selectedSearchEngineUrl.value) || searchEngineList[0]
  const config = { currentSearchEngine: engine }
  searchConfigSaving.value = true
  try {
    const { code, msg } = await setSearchConfig(selectedSpaceId.value, config)
    if (code === 0) {
      const cache = readSpaceCache(selectedSpaceId.value, authStore.userInfo?.id)
      if (cache) {
        cache.searchConfig = config
        writeSpaceCache(selectedSpaceId.value, cache, authStore.userInfo?.id)
      }
      window.dispatchEvent(new CustomEvent('yin-panel-search-config-saved', { detail: { spaceId: selectedSpaceId.value, config } }))
      message.success(t('apps.baseSettings.searchEngineSaved'))
    }
    else {
      message.error(`${t('apps.baseSettings.searchEngineSaveFailed')}: ${msg}`)
    }
  }
  finally {
    searchConfigSaving.value = false
  }
}
const searchEngineOptions = searchEngineList.map(engine => ({
  label: engine.title,
  value: engine.url,
}))
onMounted(load)
</script>

<template>
  <div class="p-4">
    <NCard :title="$t('spaceManage.title')">
      <NSpace vertical>
        <NSpace>
          <NSelect v-model:value="selectedSpaceId" data-testid="space-select" class="min-w-[220px]" :options="spaceOptions(spaces, authStore.userInfo?.id)" @update:value="(id) => selectSpace(Number(id))" />
          <NButton type="primary" @click="openCreateDialog">{{ $t('spaceManage.createSpace') }}</NButton>
        </NSpace>
        <NModal v-model:show="createDialogVisible" preset="dialog" :title="$t('spaceManage.createSpace')" :positive-text="$t('common.confirm')" :negative-text="$t('common.cancel')" :loading="loading" @positive-click="create">
          <NInput v-model:value="name" :placeholder="$t('spaceManage.newSpaceName')" maxlength="100" @keyup.enter="create" />
        </NModal>

        <NCard v-if="selectedSpaceId" size="small" :title="$t('spaceManage.sectionBasic')">
          <div class="mb-2 text-xs opacity-70">{{ $t('spaceManage.scopeHint') }}</div>
          <NSpace>
            <NButton @click="openRenameDialog">{{ $t('spaceManage.rename') }}</NButton>
            <NButton @click="copyCurrent">{{ $t('spaceManage.copy') }}</NButton>
          </NSpace>
          <div v-if="editableSpace" class="mt-3 max-w-[280px]">
            <div class="mb-1">{{ $t('themeScope.spaceTitle') }}</div>
            <NSelect v-model:value="spaceThemeSelection" :options="spaceThemeOptions" />
            <NButton class="mt-2" size="small" type="primary" :loading="spaceThemeSaving" @click="saveSpaceTheme">
              {{ $t('common.save') }}
            </NButton>
          </div>
          <NModal v-model:show="renameDialogVisible" preset="dialog" :title="$t('spaceManage.rename')" :positive-text="$t('common.confirm')" :negative-text="$t('common.cancel')" @positive-click="renameCurrent">
            <NInput v-model:value="rename" :placeholder="$t('spaceManage.spaceName')" maxlength="100" @keyup.enter="renameCurrent" />
          </NModal>
        </NCard>

        <NCard v-if="selectedSpaceId" size="small" :title="$t('spaceManage.sectionContent')">
          <div class="w-full">
            <div class="flex items-center gap-2"><span>{{ $t('spaceManage.groups') }}</span><NButton size="tiny" type="primary" @click="openAddGroup(selectedSpaceId!)">{{ $t('spaceManage.addGroup') }}</NButton><span v-if="!groups[selectedSpaceId]?.length" class="yin-text-muted">{{ $t('common.noData') }}</span></div>
            <NSpace vertical size="small" class="mt-1"><NButton v-for="group in groups[selectedSpaceId]" :key="group.id" size="tiny" secondary @click="editGroup(selectedSpaceId!, group)">{{ group.title }}{{ group.parentId ? ` ${$t('spaceManage.childGroupLabel')}` : '' }}</NButton></NSpace>
          </div>
          <NModal v-model:show="groupDialogVisible" preset="dialog" :title="editing ? $t('common.edit') : $t('spaceManage.addGroup')" :positive-text="$t('common.confirm')" :negative-text="$t('common.cancel')" @positive-click="saveGroup(selectedSpaceId!)">
            <NSpace vertical>
              <NInput v-model:value="groupName" :placeholder="$t('spaceManage.groupName')" maxlength="100" @keyup.enter="saveGroup(selectedSpaceId!)" />
              <NSelect v-model:value="groupParentId" clearable :placeholder="$t('spaceManage.parentGroupOptional')" :options="(groups[selectedSpaceId] || []).filter(group => group.id !== editing?.id).map(group => ({ label: group.title, value: group.id }))" />
            </NSpace>
          </NModal>

          <div class="mt-3 max-w-[280px]">
            <div class="mb-1">{{ $t('apps.baseSettings.searchEngine') }}</div>
            <div class="flex items-center">
              <NSelect v-model:value="selectedSearchEngineUrl" to="body" :options="searchEngineOptions" />
              <NButton class="ml-[10px]" type="primary" :loading="searchConfigSaving" @click="saveDefaultSearchEngine">
                {{ $t('apps.baseSettings.saveSearchEngine') }}
              </NButton>
            </div>
          </div>

          <div class="mt-3">
            <div class="mb-1">{{ $t('spaceManage.bookmarkTransfer') }}</div>
            <NSpace><NButton data-testid="bookmark-export" @click="exportBookmarks">{{ $t('spaceManage.exportBookmarks') }}</NButton><NButton data-testid="bookmark-choose-file" @click="bookmarkFileInput?.click()">{{ $t('spaceManage.chooseFile') }}</NButton><input ref="bookmarkFileInput" data-testid="bookmark-file-input" class="hidden" type="file" accept=".html,text/html" @change="previewBookmarks"></NSpace>
            <NList v-if="bookmarkPreview" class="mt-2"><NListItem v-for="(row, index) in bookmarkPreviewRows" :key="index" data-testid="bookmark-preview-row"><span :style="{ paddingLeft: `${row.depth * 16}px` }">{{ row.title }} {{ $t('spaceManage.bookmarkGroupCount', { n: row.count }) }}</span></NListItem><NProgress v-if="importing" type="line" :percentage="importProgress" processing /><NButton data-testid="bookmark-import-confirm" type="primary" :loading="importing" @click="confirmImport">{{ $t('spaceManage.confirmImport') }}</NButton></NList>
          </div>
        </NCard>

        <NCard v-if="selectedSpaceId" size="small" :title="$t('spaceManage.sectionSharing')">
          <div>
            <div class="yin-text-muted font-bold">{{ $t('spaceManage.publicAccess') }}</div>
            <NSpace vertical class="mt-1">
              <label><input v-model="publicEnabled" type="checkbox"> {{ $t('spaceManage.enablePublicAccess') }}</label>
              <NInput v-model:value="publicId" :placeholder="$t('spaceManage.publicIdPlaceholder')" maxlength="30" />
              <NSelect v-model:value="publicMode" :options="[{ label: $t('spaceManage.directAccess'), value: 'direct' }, { label: $t('spaceManage.codeAccess'), value: 'code' }]" />
              <NInput v-if="publicMode === 'code'" v-model:value="publicAccessCode" type="password" show-password-on="click" :placeholder="$t('spaceManage.accessCodePlaceholder')" maxlength="12" />
              <span v-if="publicEnabled && publicId" class="yin-text-muted">{{ $t('spaceManage.publicAddress') }}{{ `${publicOrigin}/${publicId}` }}</span>
              <NButton type="primary" :loading="publicSaving" @click="savePublic">{{ $t('spaceManage.savePublic') }}</NButton>
            </NSpace>
          </div>

          <div class="mt-4">
            <div class="flex items-center gap-2"><span class="yin-text-muted font-bold">{{ $t('spaceManage.members') }}</span><NButton size="tiny" type="primary" @click="openAddMember">{{ $t('spaceManage.addMember') }}</NButton></div>
            <NList><NListItem v-for="member in members" :key="member.id"><NSpace justify="space-between" class="w-full" align="center"><span>{{ memberLabel(member) }} ({{ member.source || 'manual' }})</span><NSpace align="center"><NSelect class="w-32" :value="member.role" :disabled="isSpaceOwner(member)" :options="[{ label: $t('spaceManage.admin'), value: 'admin' }, { label: $t('spaceManage.editor'), value: 'editor' }, { label: $t('spaceManage.viewer'), value: 'viewer' }]" @update:value="role => changeMember(member, role)" /><NButton v-if="!isSpaceOwner(member)" size="tiny" @click="removeCurrentMember(member)">{{ $t('spaceManage.removeMember') }}</NButton></NSpace></NSpace></NListItem></NList>
          </div>
          <NModal v-model:show="memberDialogVisible" preset="dialog" :title="$t('spaceManage.addMember')" :positive-text="$t('common.confirm')" :negative-text="$t('common.cancel')" @positive-click="addCurrentMember">
            <NSpace vertical>
              <NInput v-model:value="memberEmail" :placeholder="$t('spaceManage.userEmail')" @keyup.enter="addCurrentMember" />
              <NSelect v-model:value="memberRole" :options="[{ label: $t('spaceManage.editor'), value: 'editor' }, { label: $t('spaceManage.viewer'), value: 'viewer' }]" />
            </NSpace>
          </NModal>

          <div class="mt-4">
            <div class="flex items-center gap-2"><span class="yin-text-muted font-bold">{{ $t('spaceManage.oidcTitle') }}</span><NButton size="tiny" type="primary" @click="openAddRule">{{ $t('spaceManage.addRule') }}</NButton></div>
            <NList><NListItem v-for="rule in oidcRules" :key="rule.id"><NSpace justify="space-between" class="w-full"><span>{{ rule.provider }} / {{ rule.groupName }} ({{ rule.role }})</span><NButton size="small" @click="removeRule(rule.id)">{{ $t('common.delete') }}</NButton></NSpace></NListItem></NList>
          </div>
          <NModal v-model:show="oidcDialogVisible" preset="dialog" :title="$t('spaceManage.addRule')" :positive-text="$t('common.confirm')" :negative-text="$t('common.cancel')" @positive-click="addRule">
            <NSpace vertical>
              <NInput v-model:value="oidcProvider" placeholder="Provider" />
              <NInput v-model:value="oidcGroup" :placeholder="$t('spaceManage.groupName')" @keyup.enter="addRule" />
              <NSelect v-model:value="oidcRole" :options="[{ label: $t('spaceManage.editor'), value: 'editor' }, { label: $t('spaceManage.viewer'), value: 'viewer' }]" />
            </NSpace>
          </NModal>
        </NCard>

        <NCard v-if="selectedSpaceId" size="small" :title="$t('spaceManage.sectionDanger')">
          <div class="mb-2 text-xs opacity-70">{{ $t('spaceManage.dangerHint') }}</div>
          <div class="flex flex-wrap items-center gap-2">
            <NButton type="error" secondary @click="clearCurrentSpace">{{ $t('spaceManage.clearSpace') }}</NButton>
            <NButton v-if="canLeaveSpace()" type="error" secondary @click="leaveCurrentSpace">{{ $t('spaceManage.leaveSpace') }}</NButton>
          </div>
        </NCard>
      </NSpace>
    </NCard>
  </div>
</template>
