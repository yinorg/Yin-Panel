// The standard home layout: a flat grid of groups. It renders no chrome of its
// own, so every hook is a no-op and the shell's shared behaviour applies.
const yinHomeLayout = {
  kind: 'standard',
  collectionGap: 24,
  allowDragEdit: true,
  hideEmptyGroups: false,
  createNav: () => null,
  refreshNav: () => {},
  visibleGroups: groups => (groups || []).map(group => ({ group, depth: 0 })),
  decorateGroup: () => {},
  itemChrome: presentation => ({
    hideTitle: presentation?.iconTextIconHideTitle === true,
    hideDescription: presentation?.iconStyle === 'info' ? presentation.iconTextInfoHideDescription === true : true,
    info: presentation?.iconStyle === 'info',
  }),
}
