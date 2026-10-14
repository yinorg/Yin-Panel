# Theme System

## Core and Theme Responsibility Boundary

The home page is rendered by a theme package; the Core supplies what a theme cannot safely do for itself.

| # | Core responsibility | Content |
| --- | --- | --- |
| C1 | Data channel | Talks to the backend, fetches and mutates data, projects it into the snapshot handed to the theme. A theme never talks HTTP. |
| C2 | Policy and trust boundary | Holds the session token (httpOnly cookie), enforces a permission per command, validates arguments before acting. The backend is the ceiling: a theme can never exceed the acting user's rights. |
| C3 | Bootstrap and degradation | Startup loading state, the theme consent prompt, and the minimum viable fallback view for when the theme cannot render. |
| C4 | Application shell | Login, public access codes, theme recovery, settings and admin surfaces — everything that is not the home's own rendering. |

The theme owns all presentation and interaction of the home page: layout, styling, and which affordances it exposes.

**Data access uses typed capability commands.** A theme never calls HTTP directly; it goes through `api.commands.execute('item.create', { ... })`. Each command declares the permission it needs and the Core enforces it, so the backend HTTP shape is not part of the theme contract and can evolve freely. The snapshot handed to the theme is scoped by the same permission set (`scopedSnapshot` in `ThemeHost.vue`), so a theme without a grant renders read-only and empty rather than leaking data it was never allowed to see.

**Surfaces the Core keeps.** A theme invokes them through commands; the Core renders them:

| Command | Core surface | Why it stays in Core |
| --- | --- | --- |
| `editor.open` | Item editor dialog | Needs file upload and icon selection, i.e. privileged operations |
| `commandCenter.open` | Command centre | Cross-module navigation and service aggregation |
| `ui.openCoreSurface(name)` | A Core page the theme may cover: `apps`, `account`, `appearance`, `layout`, `wallpaper-center`, `space-manage`, `instance`, `users`, `about` | The theme renders it on its own route when it contributes the view; otherwise the Core settings modal opens on the matching app |

The settings pages were split by scope — the user's own account (`account`), the home's look (`appearance`), its structure and component switches (`layout`), the selected space (`space-manage`), and the whole instance (`instance`, admins only). The pre-split names `user-info` and `theme-settings` stay accepted and resolve to `account` and `appearance`, so a theme written against the old contract keeps working.

**Theme-contributed surfaces render on their own route.** A theme may contribute views beyond the home (`theme-settings`, `theme-page`, or any named view its manifest declares). `ui.openCoreSurface(name)` navigates to `/theme/<name>`, where the Core mounts that view into the same runtime the home uses (`views/home/ThemeSurface.vue`); the Core renders no page of its own there. When the theme does not contribute the view, or has fallen back, the visitor is returned to the home. The backend serves the SPA entry document for `/theme/:view` so the route deep-links and survives a reload. The theme's light-DOM scope class is applied to the surface container, so the theme's own styles and tokens reach the page. Protected surfaces — login, public access code, theme recovery, and the C3 fallback — stay Core-only and are never routed through a theme. Only a same-origin (direct-mounted) theme can render a surface: a sandboxed theme cannot mount into the Core document, so its `ui.openCoreSurface` calls fall back to the Core surface instead. The Core's own entries use the same path: the command centre's `settings` entry opens `apps`, which the theme renders as its app hub (and falls back to the Core modal when the theme does not contribute it).

**Decision rule for future work.** Needs a privilege (token, file, routing) or must exist when no theme is present → Core. Pure presentation and interaction → theme.

**Wallpapers are Core-managed; a theme may contribute a catalogue.** The picker ("wallpaper centre"), the layer that paints a choice, and the stored selection are all Core capabilities, because each needs something a theme does not hold: packaged artwork served by the Core's own asset route (a `page` entry is `text/html` and is served with a `sandbox allow-scripts` policy), a preference persisted per user on the server, and one shared surface every theme opens the same way. A theme opens it with `api.ui.openCoreSurface('wallpaper-center')` or embeds `<yin-wallpaper-center>`; it never builds a second picker. Content comes from two places: the Core's built-in library (`backend/internal/biz/theme/wallpaperpack`, a versioned pack of categories whose entries are an `image` or a sandboxed `page`) and, optionally, one catalogue the theme declares in `manifest.wallpaperCatalogue` — a packaged resource of the same shape, whose entries are namespaced with the package id. The panel config stores only an entry id (`wallpaperMode: 'library'` plus `wallpaperLibraryEntry`), the backend validates it against the library when it is written, and the layer falls back to the theme's own wallpaper when an entry no longer resolves. Registered element: `<yin-wallpaper-center>`.

**Where the home shell sits.** `views/home/index.vue` is the host: it wires the `core/home/use*` composables (environment, monitor, theme runtime, data, modals, public access, commands) and renders only the Core-owned chrome around the theme. A theme mounts same-origin (trusted); where a legacy iframe is used the Core cannot measure inside it, so the theme reports its own geometry back through `layout.report` for the monitor band.

## Theme Execution, Trust, and the Component Bridge

**Themes are same-origin, trusted applications ("theme = app").** A theme is code the operator explicitly installed and trusts, the way VS Code extensions and WordPress themes are. This is a deliberate trade: it lets a theme render the whole home page and reuse the Core's own components, at the cost of "a theme can act as the user". Consequently the security boundary is three layers, not a sandbox:

1. **Install trust (front door).** Installing a theme, and every later version, is an explicit act by the user/operator. The marketplace reviews and signs packages and **re-reviews version updates** so a theme cannot silently gain behaviour; revocation is always available.
2. **Backend ceiling (hard).** The backend authorizes every request against the acting user's rights. A theme can never exceed the user's own permissions, reach another tenant's private data, or affect the server. It cannot escalate privilege; it can only do what the user could do.
3. **Frontend hardening (damage reduction).** The session token lives in an **httpOnly cookie** so theme code cannot read credentials, and a strict **CSP** (`connect-src`/`img-src 'self'`) blocks exfiltration. These are not a sandbox; together they bound a malicious theme to "acting as the user inside the app".

Two consequences must be stated plainly:

- Backend enforcement cannot tell "the user clicked delete" from "the theme called delete". Protecting a user from a theme **they installed** is out of scope; protecting the server, other tenants, and credentials is in scope.
- Third-party themes default to the same trusted treatment (**install = trust**). A package that wants isolation may still be published as an untrusted theme that runs in the legacy frame — that path is kept for that case and is no longer the default.

**Core component bridge.** So themes never re-implement Core UI, the Core registers its home components as **custom elements** (for example `<yin-system-monitor>`, later `<yin-item-editor>`). A theme uses them declaratively and gets the exact Core rendering with zero duplication; the built-in Yin theme consumes the same elements, which is how it stays pixel-identical across releases. Rollout is staged: the monitor element first, other surfaces follow. The monitor element is live: `<yin-system-monitor>` mounts the same `SystemMonitor` component as the Core band, shares the Core's poller (refcounted), and — because the theme mounts into the light DOM — inherits the theme's tokens and CSS. Attributes `show-title` and `icon-text-color` override the Core preferences. The built-in theme renders the monitor itself on the home, in normal flow; the Core's own band steps aside while a `<yin-system-monitor>` is connected and stays only as the fallback for a theme that does not embed it (`?yinCoreMonitor=1` forces the Core band for comparison). Registered elements: `<yin-system-monitor>`, `<yin-account>`, `<yin-appearance>`, `<yin-layout>`, `<yin-space-manage>`, `<yin-instance>`, `<yin-users>`, `<yin-about>`. `<yin-user-info>` and `<yin-style>` remain registered for themes written before the settings pages were split, and resolve to the account and appearance pages. An element forwards its component's emits back to the Core through the bridge — for example `spaces-changed` reloads the Core's spaces — so an action taken on a theme-rendered Core page keeps the Core in step.

## Theme Resolution and Scoping

Which theme a visitor sees is decided by **one server-side resolution**, never by the client. Precedence, highest first:

1. the user's per-space override (reserved; not yet exposed);
2. the user's own choice (`mode = custom`);
3. the space's theme (`mode = follow-space`, and any user without a choice);
4. the system default;
5. the built-in default (`org.yin.default`).

| Actor | May set |
| --- | --- |
| System administrator | the system default; own preference |
| Space administrator (owner/admin) | that space's theme; own preference |
| Editor / viewer | own preference |
| Guest (public link) | nothing — resolves to the space theme, then the system default |

**Empty means inherit.** A space or user with no explicit theme follows the system default dynamically, so changing the default reaches every space that has not overridden it. New accounts default to `follow-space`; existing preferences migrate to `custom`.

**Failure fallback.** If the resolved theme cannot run (removed or retired, an unavailable mode, consent refused) resolution moves one step down the chain and reports why. **Guest resolution skips trusted-only or consent-requiring themes.**


## Governance: Review, Signing, Version Re-Review, Revocation

The install-trust layer is enforced by these mechanisms:

- **Signing and review.** A package is signed by its publisher and reviewed before publication. `ParsePackageArchiveV2` accepts an unsigned package only with an explicit `confirmUnverified`; the admin UI shows a verified/unverified badge and prompts before installing an unsigned one, so the operator always knows what they are trusting.
- **Version re-review.** Trust is granted per revision, not per package: a new revision needs its own grant, and enabling the trusted runtime is a separate administrator decision per revision. A theme cannot gain behaviour by shipping an update.
- **Revocation.** Removing a package marks it removed, reverts activations to the built-in default, and resolution skips any space or per-user preference that points at it, so a revoked theme falls back instead of staying pinned.
- **Audit.** Install, select, remove, and trusted-policy changes are recorded.

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

## Data Layer and Rendering Layer

The boundary above has one consequence worth stating on its own: **the Core supplies data, the theme renders it.** Two pieces of data exist for that reason, and a theme may use, ignore, or replace what it is given.

### The wallpaper's tone

The Core owns the wallpaper, so it is the only side that can read it. It samples the artwork it paints and hands the result over in the snapshot:

```ts
snapshot.wallpaper?: {
  /** How light the artwork is; null when it is painted but cannot be read
   *  (a sandboxed page, a cross-origin frame, a picture that taints the canvas). */
  tone: 'light' | 'dark' | null
  /** What the Core would use. Not a rule — a theme may ignore it. */
  onWallpaper?: string
  onWallpaperMuted?: string
}
```

The field is absent when nothing is painted. A theme that wants different colours derives them from `tone` itself; a theme that wants none ignores the field entirely. The built-in home packages read `onWallpaper` for every piece of text they paint on the artwork, so one value drives the clock, group titles and affordances together instead of each one hardcoding white.

### Monitor samples arrive as events

The system monitor refreshes on its own cadence, so its samples are announced rather than carried in the snapshot — folding them in would re-announce `state.updated` on every tick and make the theme re-render the whole home:

```js
const snapshot = await api.monitor.getSnapshot()
const stop = api.events.subscribe('monitor.updated', event => draw(event.payload.monitor))
```

`ThemeMonitorSnapshot` carries `capturedAt`, `cpu`, `memory` and `network`.

### Where the monitor band belongs

The band is **presentation**, so it belongs to the theme. The Core registers `<yin-system-monitor>` as a component bridge for themes that would rather not draw it, and keeps its own layer as the minimum viable fallback when no theme renders one — but a home package that ships its own band owns its markup, its styles and its colours. That is what makes the band follow the wallpaper tone like every other surface instead of being pinned to white by a Core stylesheet.

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
