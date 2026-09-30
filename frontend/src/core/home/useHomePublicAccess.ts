import { computed, ref } from 'vue'
import { getSpaces } from '@/api/panel/space'
import type { Space } from '@/api/panel/space'

const ACCESS_CODE_MIN_LENGTH = 4
const ACCESS_CODE_MAX_LENGTH = 12

/**
 * Public-link access flow.
 *
 * A public link may be configured for direct access, in which case the server
 * never asks for a code. Only the server knows the mode, so this starts in a
 * checking state and asks `/api/spaces` (reachable with the public code header)
 * before showing any prompt. `publicAccessReady` is the Core's own decision that
 * the visitor may see this space; it gates both the data load and the theme's read
 * scopes, so a `code` link must not report ready before the visitor unlocks it.
 */
export function useHomePublicAccess(input: {
  /** The public code parsed from the path, or '' for a normal signed-in visit. */
  publicCode: string
  /** Lazy because the data composable owns this state and is created after this
   *  one — it needs `publicAccessReady` from here, so the two are mutually
   *  dependent and must be read through a getter. */
  getActiveSpace: () => Space | null
  loadHomeData: () => void
}) {
  const publicAccessCode = ref('')
  const publicAccessReady = ref(!input.publicCode || !!sessionStorage.getItem(`yin-panel-public-access:${input.publicCode}`))
  const publicAccessChecking = ref(!!input.publicCode && !publicAccessReady.value)

  async function resolvePublicAccessMode() {
    if (!input.publicCode || publicAccessReady.value) {
      publicAccessChecking.value = false
      return
    }
    try {
      const { code, data } = await getSpaces<Space[]>()
      if (code === 0 && Array.isArray(data) && data[0]?.publicMode !== 'code')
        publicAccessReady.value = true
    }
    catch {
      // Fall through to the access-code prompt.
    }
    finally {
      publicAccessChecking.value = false
    }
  }

  function unlockPublicAccess() {
    if (publicAccessCode.value.length < ACCESS_CODE_MIN_LENGTH || publicAccessCode.value.length > ACCESS_CODE_MAX_LENGTH) return
    sessionStorage.setItem(`yin-panel-public-access:${input.publicCode}`, publicAccessCode.value)
    publicAccessReady.value = true
    input.loadHomeData()
  }

  // A public link or a viewer membership must never expose write affordances. The
  // server rejects the writes anyway, but the buttons still render and open dead
  // dialogs, so gate them here too.
  const canEditActiveSpace = computed(() => !input.publicCode && input.getActiveSpace()?.canEdit !== false)

  return {
    publicAccessCode,
    publicAccessReady,
    publicAccessChecking,
    resolvePublicAccessMode,
    unlockPublicAccess,
    canEditActiveSpace,
  }
}
