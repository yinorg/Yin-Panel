# Theme System

## Package and Runtime Boundary

Theme packages contain a manifest, DTCG 2025.10 token documents, and declared static resources. They do not contain JavaScript, Vue components, selectors, layout definitions, or executable templates. Wallpaper metadata remains a constrained manifest resource reference; wallpaper image bytes and fonts remain static assets.

The runtime validates the package, resolves DTCG references, maps semantic and component tokens to `--yin-*` CSS custom properties, and applies the selected light or dark document. API v3 packages declare `compatibility.engine`, `compatibility.minimum`, and optional `compatibility.maximum`. API v1 and v2 packages remain readable through their semantic bindings; v2 layout slots are ignored. DTCG remains `2025.10` for every API version.

The user configuration owns `homeLayout`, content width, page margins, responsive breakpoints, and grid structure. Theme tokens own visual density values such as card padding, group gaps, icon size, and section spacing. Changing a theme does not write to panel configuration.

## DTCG Layers

```text
primitive.color.*
  -> semantic.color.*, semantic.typography.*, semantic.surface.*, semantic.state.*
    -> component.appIcon.*, component.card.*, component.group.*,
       component.searchBox.*, component.sidebar.*, component.dialog.*,
       component.menu.*, component.button.*, component.input.*,
       component.tooltip.*, component.systemMonitor.*
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

## Official Directions

`org.yin.default` retains the Yin visual direction. `org.yin.glass` uses frosted surfaces, blur, larger radii and glow. `org.yin.minimal` uses small radii, spacious density, low motion and restrained elevation. `org.yin.cyber` uses monospace typography, compact density, double outlines, neon elevation and a grid texture. Mist and Horizon remain installed alternatives and use the same v3 contract. Horizon's directory layout is a separate panel preference.

Run `npm run audit:theme-tokens` to discover remaining literal visual declarations in Vue style blocks and CSS/Less. Brand marks, user-authored item colors, data visualization colors, code highlighting, and markdown renderer styles need an explicit product decision before being converted.
