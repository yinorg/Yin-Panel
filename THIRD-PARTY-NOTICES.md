# Third-Party Notices

Yin-Panel includes the third-party software and assets listed below. Their
licenses continue to apply to those portions, and the required notices are
reproduced here.

This file is part of every distribution of Yin-Panel. See [LICENSE](./LICENSE)
for the license of Yin-Panel itself.

## sun-panel

Yin-Panel is derived from [sun-panel](https://github.com/hslr-s/sun-panel) and
from [PhantomMaa's fork](https://github.com/PhantomMaa/sun-panel) of it. Both
upstream repositories distribute sun-panel under the MIT License reproduced
below. Portions of this repository — including parts of the backend, the
frontend, and the directory layout — originate from that code and remain
subject to this notice.

```text
MIT License

Copyright (c) 2023 红烧猎人

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Icon sets

`frontend/public/assets/svg-icons/` contains SVG icons sourced from the
[Iconify](https://iconify.design/) catalogue. The icons are used unmodified
apart from colouring applied with CSS. All sets in use are listed below; the
sets with an attribution obligation are called out after the table.

| Icon set | License | Author | Source |
| --- | --- | --- | --- |
| Basil | CC-BY-4.0 | Craftwork | <https://www.figma.com/community/file/931906394678748246> |
| Clarity | MIT | VMware | <https://github.com/vmware/clarity> |
| Element Plus | MIT | Element Plus | <https://github.com/element-plus/element-plus-icons> |
| Font Awesome 6 Solid | CC-BY-4.0 | Dave Gandy | <https://github.com/FortAwesome/Font-Awesome> |
| Google Material Icons | Apache-2.0 | Material Design Authors | <https://github.com/material-icons/material-icons> |
| IconPark | Apache-2.0 | ByteDance | <https://github.com/bytedance/IconPark> |
| IconaMoon | CC-BY-4.0 | Dariush Habibpour | <https://github.com/dariushhpg1/IconaMoon> |
| IonIcons | MIT | Ben Sperry | <https://github.com/ionic-team/ionicons> |
| Lucide | ISC | Lucide Contributors | <https://github.com/lucide-icons/lucide> |
| Majesticons | MIT | Gerrit Halfmann | <https://github.com/halfmage/majesticons> |
| Material Design Icons | Apache-2.0 | Pictogrammers | <https://github.com/Templarian/MaterialDesign> |
| Material Line Icons | MIT | Vjacheslav Trushkin | <https://github.com/cyberalien/line-md> |
| Material Symbols | Apache-2.0 | Google | <https://github.com/google/material-design-icons> |
| MingCute Icon | Apache-2.0 | MingCute Design | <https://github.com/Richard9394/MingCute> |
| Phosphor | MIT | Phosphor Icons | <https://github.com/phosphor-icons/core> |
| Remix Icon | Apache-2.0 | Remix Design | <https://github.com/cyberalien/RemixIcon> |
| Solar | CC-BY-4.0 | 480 Design | <https://www.figma.com/community/file/1166831539721848736> |
| Tabler Icons | MIT | Paweł Kuna | <https://github.com/tabler/tabler-icons> |

### Attribution-required sets

The following sets are licensed under Creative Commons Attribution licenses and
require the attribution given above to be preserved:

- **Font Awesome 6 Solid** — CC-BY-4.0, © Dave Gandy / Font Awesome
  (<https://fontawesome.com>). Icons are CC-BY-4.0; the Font Awesome fonts are
  under the SIL OFL 1.1 and the code under MIT.
- **Basil** — CC-BY-4.0, © Craftwork.
- **IconaMoon** — CC-BY-4.0, © Dariush Habibpour.
- **Solar** — CC-BY-4.0, © 480 Design.

## DTCG format fixtures

`shared/theme/dtcg-conformance/` contains DTCG format evaluation fixtures
licensed under the W3C 3-clause BSD License. The full notice is in
[`shared/theme/dtcg-conformance/NOTICE.md`](./shared/theme/dtcg-conformance/NOTICE.md).

## Dependencies

Runtime and build dependencies are listed in `backend/go.mod`,
`frontend/package.json`, and their lock files. Third-party Go modules and
npm packages are distributed under their own licenses; run
`corepack pnpm licenses list` in `frontend/` and `go list -m -json all` in
`backend/` to inspect them.
