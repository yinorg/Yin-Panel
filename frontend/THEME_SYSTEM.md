# Theme System

## Core and Theme Responsibility Boundary

The home page is rendered by a theme package; the Core supplies what a theme cannot safely do for itself.

| # | Core responsibility | Content |
| --- | --- | --- |
| C1 | Data channel | Talks to the backend, fetches and mutates data, projects it into the snapshot handed to the theme. A theme never talks HTTP. |
| C2 | Policy and trust boundary | Holds the JWT, enforces a permission per command, validates arguments before acting. A theme never holds a token. |
| C3 | Bootstrap and degradation | Startup loading state, the theme consent prompt, and the minimum viable fallback view for when the theme cannot render. |
| C4 | Application shell | Login, public access codes, theme recovery, settings and admin surfaces — everything that is not the home's own rendering. |

The theme owns all presentation and interaction of the home page: layout, styling, and which affordances it exposes.

**Data access uses typed capability commands.** A theme never calls HTTP directly; it goes through `api.commands.execute('item.create', { ... })`. Each command declares the permission it needs and the Core enforces it, so the backend HTTP shape is not part of the theme contract and can evolve freely. The snapshot handed to the theme is scoped by the same permission set (`scopedSnapshot` in `ThemeHost.vue`), so a theme without a grant renders read-only and empty rather than leaking data it was never allowed to see.

**Surfaces the Core keeps.** A theme invokes them through commands; the Core renders them:

| Command | Core surface | Why it stays in Core |
| --- | --- | --- |
| `editor.open` | Item editor dialog | Needs file upload and icon selection, i.e. privileged operations |
| `commandCenter.open` | Command centre | Cross-module navigation and service aggregation |
| `ui.openCoreSurface('theme-settings')` | Theme settings page | Application shell (C4) |

**Decision rule for future work.** Needs a privilege (token, file, routing) or must exist when no theme is present → Core. Pure presentation and interaction → theme.

**Where the home shell sits.** `views/home/index.vue` is the host: it wires the `core/home/use*` composables (environment, monitor, theme runtime, data, modals, public access, commands) and renders only the Core-owned chrome around the theme frame. The theme renders inside a frame the Core cannot measure, so the theme reports its own geometry back through `layout.report` for the monitor band.

## Theme Trust Boundary

**The theme frame is same-origin with the panel, and a theme must therefore be treated as fully trusted code.** The frame's `sandbox` (defined in exactly one place, `src/theme/runtime/sandbox.ts`) carries `allow-scripts allow-popups allow-popups-to-escape-sandbox allow-same-origin`. Consequences, which are accepted rather than incidental:

- A theme can read the panel's `localStorage` (including the JWT), its cookies, and its DOM.
- `allow-scripts` plus `allow-same-origin` together let the frame remove its own `sandbox` attribute.
- The frame is not an OOPIF, so it shares a renderer process with the panel.

**Why `allow-same-origin` is required.** Without it the frame has an opaque origin and Chrome's site isolation puts it in a separate process (an OOPIF). DevTools device mode synthesises input with `Input.emulateTouchFromMouseEvent` (mouse-to-touch), and that path **does not deliver events into an OOPIF**. The symptom is precise and easy to misread: under device emulation a tap on the theme does nothing at all — the theme never sees a single `pointerdown` — while the same build works perfectly on a real device, which uses the ordinary input pipeline. No amount of listener changes fixes it, because the events never arrive.

**This is acceptable only while every theme is built in.** `yin` and `glass` ship from this repository and the operator trusts the code they deploy. **If third-party themes or a theme marketplace are ever introduced, this token must be re-evaluated first**: an untrusted theme must not be given it. In that case DevTools device emulation can no longer be used to accept or reject a change to theme click handling; **a real device becomes the only valid check**.


## Package and Runtime Boundary

Theme packages contain a manifest, DTCG 2025.10 token documents, declared CSS/JS entrypoints, optional component/view contributions, and static resources. JavaScript runs through the Theme API permission boundary; components and views mount through stable slots rather than relying on Core DOM selectors. Wallpaper metadata remains a constrained manifest resource reference; wallpaper image bytes and fonts remain static assets.

The runtime validates the package, resolves DTCG references, maps semantic and component tokens to `--yin-*` CSS custom properties, and applies the selected light or dark document. V1 packages declare `formatVersion: 2`, Theme API `1.0.0`, and `compatibility.engine`, `compatibility.minimum`, and optional `compatibility.maximum`. Packages from retired pre-V1 formats are rejected rather than adapted. DTCG remains `2025.10`.

The user configuration owns `homeLayout`, content width, page margins, responsive breakpoints, and grid structure. Theme tokens own visual density values such as card padding, group gaps, icon size, and section spacing. Changing a theme does not write to panel configuration.

## DTCG Layers

```text
primitive.color.*
  -> semantic.color.*, semantic.typography.*, semantic.surface.*, semantic.state.*
    -> component.appIcon.*, component.card.*, component.group.*,
       component.searchBox.*, component.sidebar.*, component.dialog.*,
       component.menu.*, component.button.*, component.input.*,
       component.tooltip.*, component.systemMonitor.*, component.state.*,
       component.surface.*, component.iconography.*
```

The same DTCG document also declares `shape`, `spacing`, `density`, `elevation`, `motion`, `background`, and `effect`. Reference resolution is cycle checked. CSS string values reject rule delimiters and control characters; finite numbers, dimensions, duration, color, font family, shadow, and cubic-Bezier values are checked before application.

## Coverage Matrix

| Visual dimension | Token families | Runtime consumers |
| --- | --- | --- |
| Color and state | `primitive.color`, `semantic.color`, `semantic.state` | Page, controls, search, status and focus |
| Typography | `semantic.typography` | Body, headings, search, dialog and item labels |
| Shape and outline | `shape`, component `radius`, `borderWidth`, `borderStyle` | Cards, icons, search, controls, menus and dialogs |
| Spacing and density | `spacing`, `density`, component `padding`, `gap`, `sectionSpacing` | Home groups, cards, icons, search and monitor |
| Elevation | `elevation`, component `shadow` | Cards, popups, menus, dialogs and icon hover |
| Surface and blur | component `surface`, `surfaceMode`, `blur`; `component.surface` | Search, switcher, Naive UI surfaces and background effects |
| Motion | `motion`, `component.state` durations, easing, scale and tilt | Common controls and AppIcon hover/press behavior |
| Iconography | `component.iconography`, `component.appIcon` | Bookmark icons, app cards and monitor icons |
| Background and assets | `background.texture`, `background.overlayOpacity`, manifest static wallpaper/font resources | Page texture, wallpaper layer and packaged fonts |
| Effects | `effect` opacity, glow, focus width and text shadow | Focus, AppIcon glow, status and headings |

| Component | Token coverage |
| --- | --- |
| AppIcon / ItemCard | Size, radius, surface, shadow, text, hover scale, tilt and motion |
| Group | Gap, section spacing, padding, heading typography and directory controls |
| SearchBox | Surface, height, radius, border, blur, shadow, spacing and typography |
| Sidebar / SpaceSwitcher | Surface, padding, radius, blur, shadow and semantic status colors |
| Dialog | Naive UI modal plus CommandCenter surface, padding, radius, shadow and typography |
| Menu / Tooltip | Naive UI radius, popup shadow, spacing and text color |
| Button / Input | Naive UI heights and radii, plus global motion and focus tokens |
| SystemMonitor | Group spacing, padding, radius, icon, text, and generated default colors |

Official packages provide both schemes for AppIcon, Card, Group, SearchBox, Sidebar, Dialog, Menu, Button, Input, Tooltip, SystemMonitor, state, surface and iconography. Search option size and gap follow the package density. Optional component tokens remain optional for third party V1 themes; consumers use the CSS defaults in `global.less` when an extension is absent. Updating built-in package versions causes installed built-ins to refresh without changing the selected theme or panel layout.

## Official Directions

`org.yin.default` retains the Yin visual direction. `org.yin.glass` uses frosted surfaces, blur, larger radii and glow. `org.yin.minimal` uses small radii, spacious density, low motion and restrained elevation. `org.yin.cyber` uses monospace typography, compact density, double outlines, neon elevation and a grid texture. Mist and Horizon remain installed alternatives and use the same v3 contract. Horizon's directory layout is a separate panel preference.

Run `npm run audit:theme-tokens` to scan home, common and desktop-module Vue template classes, bound styles and SFC style blocks, plus frontend CSS/Less. It reports literal visual values for review and fails if a configured file is missing, no consumer was scanned, or a default component variable has no consumer. Structural layout dimensions and decorative placements in `views/home/index.vue`/`CommandCenter` remain layout-owned; wallpaper transform/blur belongs to wallpaper configuration; `SystemMonitor/Edit/**` checkerboards are color-picker affordances; user-authored item/monitor colors and monitor chart states remain data-owned; brand/SVG artwork, Markdown and syntax-highlighting styles are also excluded from token migration. The report count is an inventory, not a pass/fail measure of theme quality.
