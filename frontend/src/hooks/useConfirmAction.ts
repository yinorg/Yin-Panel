import { useDialog } from 'naive-ui'
import { t } from '@/locales'

/**
 * Ask the user to confirm an action in the panel's own dialog.
 *
 * The settings pages used `window.confirm`, which blocks the page, ignores the
 * app's language and theme, and cannot say which action it is guarding. The call
 * site stays a plain check while the dialog is Naive's:
 *
 *     if (!await confirmAction({ content: t('...') })) return
 *
 * `variant: 'error'` marks the confirmations whose consequence is not reversible.
 */
export function useConfirmAction() {
  const dialog = useDialog()

  function confirmAction(options: { content: string, title?: string, positiveText?: string, negativeText?: string, variant?: 'warning' | 'error' }) {
    return new Promise<boolean>((resolve) => {
      const open = options.variant === 'error' ? dialog.error : dialog.warning
      open({
        title: options.title || t('common.warning'),
        content: options.content,
        positiveText: options.positiveText || t('common.confirm'),
        negativeText: options.negativeText || t('common.cancel'),
        // The dialog closes on either answer; the first resolution wins, so a
        // later onClose cannot turn a confirm into a cancel.
        onPositiveClick: () => { resolve(true) },
        onNegativeClick: () => { resolve(false) },
        onClose: () => { resolve(false) },
        onMaskClick: () => { resolve(false) },
      })
    })
  }

  return { confirmAction }
}
