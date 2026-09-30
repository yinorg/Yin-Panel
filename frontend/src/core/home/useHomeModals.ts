import { ref, type Ref } from 'vue'
import { useMessage } from 'naive-ui'
import { createGroup, createItem, createItemWithIcon, createSpace, deleteGroup, deleteItem as deleteSpaceItem, sortGroups, sortItems, updateGroup, updateItem, type Space } from '@/api/panel/space'
import { uploadImage } from '@/api/panel/file'
import { usePanelState } from '@/store'
import { t } from '@/locales'
import { PanelStateNetworkModeEnum } from '@/enums'
import { createHomeMutationService } from '@/core/home/mutations'
import { resolveItemOpenUrl } from '@/core/items/openPolicy'

/**
 * State and submissions for every dialog the home owns, plus the mutation service
 * those dialogs write through.
 *
 * `openPage` lives here rather than in a navigation helper because one of its
 * three open modes *is* a dialog (mode 3 = open inside the in-app window), so the
 * modal state and the open path are the same concern.
 */
export function useHomeModals(input: {
  activeSpace: Ref<Space | null>
  canWrite: Ref<boolean>
  reloadSpaces: (selectLatest?: boolean) => void
  refreshCollection: (forceRefresh?: boolean) => void
  invalidateSpaceCache: (spaceId: number) => void
}) {
  const panelState = usePanelState()
  const ms = useMessage()

  const editItemInfoShow = ref<boolean>(false)
  const editItemInfoData = ref<Panel.ItemInfo | null>(null)
  const windowShow = ref<boolean>(false)
  const windowSrc = ref<string>('')
  const windowTitle = ref<string>('')
  const windowIframeIsLoad = ref<boolean>(false)
  const currentAddItenIconGroupId = ref<number | undefined>()

  const settingModalShow = ref(false)
  const createSpaceVisible = ref(false)
  const spaceName = ref('')
  const creatingSpace = ref(false)
  const groupCreateVisible = ref(false)
  const groupName = ref('')
  const creatingGroup = ref(false)

  const homeMutations = createHomeMutationService({
    api: {
      createItem: (spaceId, inputPayload) => createItem<Panel.ItemInfo>(spaceId, inputPayload),
      createItemWithIcon: (spaceId, inputPayload, file) => createItemWithIcon<Panel.ItemInfo>(spaceId, inputPayload, file),
      uploadItemIcon: file => uploadImage(file),
      updateItem: (spaceId, itemId, inputPayload) => updateItem<Panel.ItemInfo>(spaceId, itemId, inputPayload),
      deleteItem: (spaceId, itemId) => deleteSpaceItem<{ code: number, msg?: string }>(spaceId, itemId),
      reorderItems: (spaceId, groupId, itemIds) => sortItems<{ code: number, msg?: string }>(spaceId, groupId, itemIds.map((id, index) => ({ id, sort: index + 1 }))),
      createGroup: (spaceId, inputPayload) => createGroup<{ code: number, msg?: string }>(spaceId, inputPayload.title, inputPayload.icon, inputPayload.parentId),
      updateGroup: (spaceId, groupId, inputPayload) => updateGroup<{ code: number, msg?: string }>(spaceId, groupId, inputPayload.title, inputPayload.icon, inputPayload.parentId),
      deleteGroup: (spaceId, groupId) => deleteGroup<{ code: number, msg?: string }>(spaceId, groupId),
      reorderGroups: (spaceId, parentId, groupIds) => sortGroups<{ code: number, msg?: string }>(spaceId, parentId, groupIds.map((id, index) => ({ id, sort: index + 1 }))),
    },
    getActiveSpaceId: () => input.activeSpace.value?.id,
    canWrite: () => input.canWrite.value,
    invalidateSpace: input.invalidateSpaceCache,
    refreshSpace: spaceId => input.activeSpace.value?.id === spaceId ? input.refreshCollection(true) : undefined,
  })

  function openPage(openMethod: number, url: string, title?: string) {
    switch (openMethod) {
      case 1:
        window.location.href = url
        break
      case 2:
        window.open(url)
        break
      case 3:
        windowShow.value = true
        windowSrc.value = url
        windowTitle.value = title || url
        windowIframeIsLoad.value = true
        break

      default:
        break
    }
  }

  function getItemOpenUrl(item: Panel.ItemInfo, forceWan = false): string {
    const userAgent = typeof navigator === 'undefined' ? '' : navigator.userAgent
    const isMobile = /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini|Mobile|Tablet/i.test(userAgent)
    const networkMode = panelState.networkMode === PanelStateNetworkModeEnum.lan ? 'lan' : 'wan'
    return resolveItemOpenUrl(item, { networkMode, isMobile, forceWan })
  }

  function handWindowIframeIdLoad(_payload: Event) {
    windowIframeIsLoad.value = false
  }

  function handleEditItem(item: Panel.ItemInfo) {
    if (!input.canWrite.value) return
    editItemInfoData.value = item
    editItemInfoShow.value = true
    currentAddItenIconGroupId.value = undefined
  }

  function handleAddItem(itemIconGroupId?: number) {
    if (!input.canWrite.value) return
    if (!input.activeSpace.value) return
    editItemInfoData.value = null
    currentAddItenIconGroupId.value = itemIconGroupId
    editItemInfoShow.value = true
  }

  function submitCreateGroup() {
    if (!input.canWrite.value) return
    const title = groupName.value.trim()
    if (!title || !input.activeSpace.value || creatingGroup.value) return
    creatingGroup.value = true
    void homeMutations.createGroup({ title, icon: '', parentId: null }).then(() => {
      groupName.value = ''
      groupCreateVisible.value = false
    }).catch((error: unknown) => {
      ms.error(`${t('common.saveFail')}:${error instanceof Error ? error.message : ''}`)
    }).finally(() => { creatingGroup.value = false })
  }

  async function submitCreateSpace() {
    if (!input.canWrite.value) return
    const name = spaceName.value.trim()
    if (!name || creatingSpace.value) return
    creatingSpace.value = true
    createSpace<{ code: number }>(name).then(({ code }) => {
      if (code === 0) { createSpaceVisible.value = false; spaceName.value = ''; input.reloadSpaces(true) }
    }).finally(() => { creatingSpace.value = false })
  }

  /** LAN/WAN is a local display/navigation preference, so switching it is a
   *  modal-layer action rather than a data mutation. */
  function handleChangeNetwork(mode: PanelStateNetworkModeEnum) {
    panelState.setNetworkMode(mode)
    if (mode === PanelStateNetworkModeEnum.lan)
      ms.success(t('panelHome.changeToLanModelSuccess'))

    else
      ms.success(t('panelHome.changeToWanModelSuccess'))
  }

  return {
    editItemInfoShow,
    editItemInfoData,
    windowShow,
    windowSrc,
    windowTitle,
    windowIframeIsLoad,
    currentAddItenIconGroupId,
    settingModalShow,
    createSpaceVisible,
    spaceName,
    creatingSpace,
    groupCreateVisible,
    groupName,
    creatingGroup,
    homeMutations,
    openPage,
    getItemOpenUrl,
    handWindowIframeIdLoad,
    handleEditItem,
    handleAddItem,
    submitCreateGroup,
    submitCreateSpace,
    handleChangeNetwork,
  }
}
