/**
 * Default panel footer markup.
 *
 * Kept in a dependency-free leaf module because two very different consumers
 * need the same value: the panel store (Core home) and the theme home snapshot
 * (sandboxed theme). The snapshot is executed directly by the unit-test runner,
 * which cannot resolve `@/` aliases, so this must stay importable by relative
 * path with no imports of its own.
 *
 * When a stored config row predates the field, both homes must still render the
 * same footer; previously the snapshot fell back to an empty string while the
 * store fell back to this markup, so the footer vanished when a theme took over.
 */
export const defaultFooterHtml = '<div class="flex justify-center text-slate-300" style="margin-top:100px">Powered By <a href="https://github.com/yinorg/Yin-Panel" target="_blank" class="ml-[5px]">Yin-Panel</a></div>'
