# Electron Locust Hover & Cursor Map (2026-11-26)

Read-only code reference for every `:hover`, `:focus-visible`, and `cursor:` rule on pressable controls in Electron's Locust (`apps/desktop/src/renderer/src/shell.css`, `tokens.css`, `components/*.tsx`), followed by a Rust/Rune audit (`apps/native/src`, `vendor/rune/crates/rune/src`).

- Electron paths are relative to `apps/desktop/src/renderer/src/`.
- Rust/Rune paths are from the newest native worktree `exec/native-fixturn-1` (`/Users/locust/work/locust-wt/opus`, commit `6ab806ba`, 2026-11-26), because `apps/native` is not on `main` in `/Users/locust/work/locust`. Note: `apps/native/vendor/rune/src` is `NOT FOUND`; Rune is vendored at repo root `vendor/rune/crates/rune/src` (`apps/native/Cargo.toml:70-74`).
- Every claim cites `file:line`. Anything absent from source is marked `NOT FOUND`.

---

## 1. The 14 Distinct Hover Looks (Electron Summary)

Token values are listed as `dark / light` from `tokens.css:203-795` (dark) and `tokens.css:817-963` (light). Motion tokens: `--lc-motion-press: 120ms`, `--lc-motion-hover: 140ms`, `--lc-ease: ease` (`tokens.css:599-602`).

| # | Hover Look | What Changes (Token & Value: Dark / Light) | Transition | Cursor | Places (`shell.css`) |
|---|---|---|---|---|---|
| 1 | **Surface wash** | `background: var(--lc-hover-surface)` (`rgba(255,255,255,0.06)` / `rgba(28,30,29,0.05)`); often paired with `color: --lc-text-secondary` (`#c7c3bc` / `#3a3e3d`), `--lc-text-primary` (`#f2efe9` / `#1c1e1d`), or `--lc-text-name` (`#f4f1eb` / `#161817`) | `140ms ease` where declared (some instant) | `pointer` | 53 rules |
| 2 | **Strong surface wash** | `background: var(--lc-hover-surface-strong)` (`rgba(255,255,255,0.07)` / `rgba(28,30,29,0.07)`); often with `color: --lc-text-primary` and `border-color: transparent` or `--lc-hover-border` | `140ms ease` (some instant) | `pointer` | 15 rules |
| 3 | **Border lift + surface wash** | `border-color: --lc-hover-border` (`rgba(255,255,255,0.2)` / `rgba(28,30,29,0.22)`) or `--lc-border-hover` (`rgba(255,255,255,0.16)` / `rgba(28,30,29,0.24)`) + `background: --lc-hover-surface`, `--lc-chip-bg`, `--lc-bg-control`, or `linear-gradient(--lc-hover-surface) + --lc-bg-raised` | `140ms ease` (120ms on `.lc-peer__bubble.is-link`) | `pointer` | 14 rules |
| 4 | **Border lift only** | `border-color` -> `--lc-border-hover`, `--lc-border-strong` (`rgba(255,255,255,0.11)` / `rgba(28,30,29,0.14)`), `--lc-hover-border`, or `--lc-lime-border-strong`; often paired with `color: --lc-text-primary` | `140ms ease` or instant | `pointer` | 21 rules |
| 5 | **Elevated surface fill (`bg-raised-2` / `bg-selected` / `bg-control`)** | `background` -> `--lc-bg-raised-2` (`#1b1e1f` / `#fcfbf9`), `--lc-bg-selected` (`#1e2223` / `#e7e6e1`), or `--lc-bg-control` (`#1d2122` / `#f2f1ed`); often with `color: --lc-text-primary` or `--lc-text-secondary` | `140ms ease` or instant | `pointer` | 26 rules |
| 6 | **Primary solid button brighten** | `background: var(--lc-primary-bg)` (`#f2efe9` / `#1c1e1d`) -> `var(--lc-primary-bg-hover)` (`#ffffff` / `#2e3130`) | `140ms ease` (transform `120ms ease`) | `pointer` | 1 rule (`.lc-primarybutton`) |
| 7 | **Composer frosted chip wash** | `background: var(--lc-chip-bg)` (`rgba(255,255,255,0.04)` / `rgba(28,30,29,0.04)`) -> `var(--lc-chip-bg-hover)` (`rgba(255,255,255,0.07)` / `rgba(28,30,29,0.07)`); optional `color: --lc-text-secondary` | instant | `pointer` | 3 rules (covers composer foot chips, `.lc-chipbutton`, open `.lc-peer__toggle`) |
| 8 | **Danger / destructive red tint** | `background: var(--lc-red-tint)` (`rgba(228,104,93,0.12)` / `rgba(201,68,58,0.1)`); `color: var(--lc-red-text)` (`#ee9188` / `#a5342b`); optional `border-color` or inset ring `var(--lc-red-border)` (`rgba(228,104,93,0.32)` / `rgba(201,68,58,0.3)`) | `140ms ease` or instant | `pointer` | 8 rules |
| 9 | **Monochrome accent tint boost** | `background: var(--lc-lime-tint-strong)` (`rgba(242,239,233,0.13)` / `rgba(28,30,29,0.11)`) + `border-color: var(--lc-lime-border-strong)` (`rgba(242,239,233,0.38)` / `rgba(28,30,29,0.4)`) | `140ms ease` (inherited from `.lc-button`/`.lc-send`) | `pointer` | 3 rules |
| 10 | **Text ink brighten only** | `color` -> `--lc-text-primary`, `--lc-text-secondary`, or `--lc-text-name`; no background or border change | `140ms ease` or instant | `pointer` | 24 rules |
| 11 | **Link underline / underline reveal** | `text-decoration: underline` (often `text-underline-offset: 2px`/`3px`) or `border-bottom-color: var(--lc-blue-text)` (`#9dc2fb` / `#1f58b0`); optional `color: --lc-text-primary` | instant | `pointer` | 11 rules |
| 12 | **Group-hover reveal / swap** | Parent `:hover` sets child `opacity: 0 -> 1`, `visibility: hidden`, or `display: none -> flex`/`grid` (action menus, edit/copy icons, checkboxes, go-arrow, jump-down dots->chevron, hover cards) | `120ms` or `140ms ease` (some instant) | `pointer` on revealed controls | 19 rules |
| 13 | **Switch track brighten** | Off-state switch `background: var(--lc-switch-off)` (`rgba(255,255,255,0.16)` / `rgba(28,30,29,0.16)`) -> `var(--lc-switch-off-hover)` (`rgba(255,255,255,0.22)` / `rgba(28,30,29,0.22)`) | instant (on base `button.lc-switch`) | `pointer` | 1 rule (`button.lc-switch`) |
| 14 | **Special / non-fill feedback** | `filter: brightness(1.15)` (`.lc-status.is-button`), `box-shadow: 0 0 0 1px var(--lc-border-hairline)` (`.lc-composer__box`), liquid rim `box-shadow` (`:root[data-look='liquid'] .lc-composer__box`), `--lc-scroll-thumb-hover` (`rgba(255,255,255,0.24)` / `rgba(28,30,29,0.32)`), `MetalSend` pointer-tracking WebGL halo (`MetalSend.tsx:330`) | `140ms ease` on `.lc-composer__box` | `pointer`, `text`, or `default` | 5 rules + 1 TSX |

*(Plus 16 rules in `shell.css` that explicitly suppress hover on static/disabled elements: `background: transparent` or `cursor: default`.)*

---

## 2. Global Focus-Visible, Cursor & Reduced-Motion Baseline

- **Global keyboard focus ring** (`tokens.css:1007-1015`): `.lc-focusable:focus-visible, button:focus-visible, [role='button']:focus-visible, [role='menuitem']:focus-visible, input:focus-visible, textarea:focus-visible` -> `outline: 2px solid var(--lc-focus-ring)` (`#f2efe9` dark / `#1c1e1d` light); `outline-offset: 2px`.
- **Pointer-click suppression** (`tokens.css:1019-1023`): `:root[data-input='pointer'] button:focus-visible, [role='button']:focus-visible, [role='menuitem']:focus-visible` -> `outline: none` (text inputs/textareas keep their ring).
- **High-contrast / forced colors** (`shell.css:22189-22193`, `@media (forced-colors: active)`): `:focus-visible` -> `outline: 2px solid Highlight; outline-offset: 2px`.
- **Reduced motion** (`tokens.css:1054-1061`, `@media (prefers-reduced-motion: reduce)`): `transition-duration: 0.001ms !important` on `*, *::before, *::after`.
- **Cursor distribution across `shell.css`** (237 `cursor:` declarations total):
  - `pointer`: 184 rules (virtually every pressable button, row, tab, chip, card, link, summary).
  - `default`: 45 rules (`:disabled` buttons/controls, `.is-static` rows, non-interactive badges).
  - `grab` / `grabbing`: `.lc-zoomimage__pane.is-pannable` (`shell.css:21627-21629`), `.lc-effortpanel__scale.is-held` (`shell.css:18368-18372`).
  - `text`: `.lc-composer__box` (`shell.css:8685-8708`).
  - `help`: `.lc-effortpanel__what` (`shell.css:18327-18342`).
  - `not-allowed`: `.lc-missionrow__pick:disabled` (`shell.css:20322-20327`).
  - `progress`: `.lc-pettile.is-busy` (`shell.css:11080-11084`).
  - `wait`: `.lc-skilllib__pack:disabled` (`shell.css:25322-25327`).
- **Scrollbar thumb** (`shell.css:18097-18102`): `::-webkit-scrollbar-thumb:hover` -> `background-color: var(--lc-scroll-thumb-hover)` (`rgba(255,255,255,0.24)` / `rgba(28,30,29,0.32)`).

---

## 3. Electron Hover, Focus-Visible & Cursor Rules by Screen

### 3.1 Window Title Bar & Top Chrome
*(Window controls rendered on Windows/Linux only; macOS uses native traffic lights, `TitleBar.tsx:51-61`.)*

| Element / Selector | Citation | Hover / Focus-Visible Change | Transition | Cursor |
|---|---|---|---|---|
| Needs-you chip (`.lc-needsyou`) | `shell.css:99-125` | `:hover` -> `border-color: --lc-border-strong` (`rgba(255,255,255,0.11)` / `rgba(28,30,29,0.14)`) | `border-color 140ms ease` | `pointer` |
| Title-bar min/max buttons (`.lc-windowcontrols button`) | `shell.css:131-149` | `:hover` -> `background: --lc-hover-surface`; `color: --lc-text-secondary` | `background 140ms ease, color 140ms ease` | `pointer` |
| Title-bar close button (`.lc-windowcontrols .lc-close`) | `shell.css:153-158` | `:hover` -> `background: --lc-red-tint`; `color: --lc-red-text` | inherits `140ms ease` | `pointer` |
| Header toolbar icon buttons (`.lc-headicon`) | `shell.css:15851-15889` | `:hover:not(:disabled)` -> `background: --lc-hover-surface`; `color: --lc-text-primary`; `:focus-visible` -> `outline: 2px solid --lc-border-strong; outline-offset: 1px` | `background 140ms ease, color 140ms ease` | `pointer` (`default` when `:disabled`) |
| Sidebar brand lockup (`.lc-brand__lockup`) | `shell.css:210-227` | `:focus-visible` -> `outline: 2px solid --lc-focus-ring; outline-offset: 2px` (no `:hover` fill) | none | `pointer` |

### 3.2 Sidebar & Compact Rail

| Element / Selector | Citation | Hover / Focus-Visible Change | Transition | Cursor |
|---|---|---|---|---|
| Sidebar place tabs (`.lc-sidebar__places button`) | `shell.css:9584-9619` | `:hover` -> `color: --lc-text-primary` | none | `pointer` |
| Sidebar footer buttons (`.lc-sidebar__footer button`) | `shell.css:510-545`, `shell.css:16992-17012` | `:hover` -> `background: --lc-hover-surface` | `background 140ms ease` | `pointer` |
| Sidebar search box (`.lc-search`) | `shell.css:288-303` | `:hover` -> `border-color: --lc-border-hover` | `background 140ms ease, border-color 140ms ease` | default/text |
| Generic sidebar row (`.lc-row`, `.lc-row--button`) | `shell.css:341-360`, `shell.css:15632-15642` | `.lc-row:hover` -> `background: --lc-bg-raised-2` (`#1b1e1f` / `#fcfbf9`) | `background 140ms ease` | `pointer` |
| Conversation row (`.lc-conv`) | `shell.css:19910-19936` | `:hover` -> `background: --lc-hover-surface`; `color: --lc-text-name` | `background 140ms ease, color 140ms ease` | `pointer` |
| Conversation row hover actions (`.lc-convrow`) | `shell.css:20064-20089` | `.lc-convrow:hover` / `:focus-within` -> `.lc-teammate__missionmenu` `opacity: 1`; `.lc-conv__age` `visibility: hidden` | `opacity 140ms ease` | `pointer` on menu |
| Conversation group header menu (`.lc-convgroup__head` / `.lc-convgroup__menu`) | `shell.css:20471-20497` | Head `:hover` / menu `:focus-visible` -> `opacity: 1`; menu `:hover` -> `background: --lc-hover-surface`; `color: --lc-text-secondary` | none | `pointer` |
| Project group header (`.lc-project__head`) | `shell.css:20413-20431` | `:hover` -> `color: --lc-text-secondary` | none | `pointer` |
| Teammate face filter button (`.lc-faces__one`) | `shell.css:20101-20121` | `:hover` -> `background: --lc-hover-surface` | none | `pointer` |
| Teammate team filter button (`.lc-faces__team`) | `shell.css:20141-20165` | `:hover` -> `background: --lc-hover-surface`; `color: --lc-text-secondary` | none | `pointer` |
| Teammate filter "more" / "clear" (`.lc-faces__more`, `.lc-faces__clearlink`) | `shell.css:20765-20785`, `shell.css:20208-20221` | `__more:hover` -> `background: --lc-hover-surface; color: --lc-text-secondary`; `__clearlink:hover` -> `color: --lc-text-name` | none | `pointer` |
| Sidebar source selector (`.lc-sidebar__source .lc-control`) | `shell.css:20183-20187` | `:hover:not(:disabled)` -> `color: --lc-text-primary` | none | `pointer` |
| "Show more" list button (`.lc-listmore`) | `shell.css:20501-20526` | `:hover` -> `background: --lc-hover-surface; color: --lc-text-name`; `:focus-visible` -> `outline: 2px solid --lc-focus-ring; outline-offset: -2px` | none | `pointer` |
| Sidebar bot badge reveal (`.lc-sidebar .lc-bot__mark`) | `shell.css:26209-26215` | `:hover > .lc-bot .lc-bot__mark`, `.lc-bot:hover .lc-bot__mark`, `:focus-visible .lc-bot__mark` -> `opacity: 1` | none | inherits |
| Compact rail flyout row / action (`.lc-railflyout__row`, `.lc-railflyout__action`) | `shell.css:18621-18641`, `shell.css:18684-18714` | `:hover` -> `background: --lc-hover-surface` | none | `pointer` |
| Update banner action (`.lc-updatebanner .lc-button`) | `shell.css:463-468` | `:hover:not(:disabled)` -> `background: --lc-lime-tint-strong`; `border-color: --lc-lime-border-strong` | inherits `140ms ease` | `pointer` |

### 3.3 Shared Controls: Buttons, Switches, Chips, Tabs & Menus

| Element / Selector | Citation | Hover / Focus-Visible Change | Transition | Cursor |
|---|---|---|---|---|
| Standard button (`.lc-button`) | `shell.css:684-713` | `:hover:not(:disabled)` -> `background: --lc-hover-surface`; `border-color: --lc-hover-border` | `background 140ms ease, border-color 140ms ease` | `pointer` (`default` when `:disabled`) |
| Primary solid button (`.lc-primarybutton`) | `shell.css:11309-11341` | `:hover:not(:disabled)` -> `background: --lc-primary-bg-hover` (`#ffffff` / `#2e3130`) | `background 140ms ease, transform 120ms ease` | `pointer` (`default` when `:disabled`) |
| Ghost / outlined button (`.lc-ghostbutton`) | `shell.css:11270-11288` | `:hover` -> `background: --lc-hover-surface-strong`; `border-color: --lc-hover-border` | `background 140ms ease` | `pointer` |
| Deny / danger button (`.lc-denybutton`) | `shell.css:15246-15268` | `:hover:not(:disabled)` -> `background: --lc-red-tint` | `background 140ms ease` | `pointer` (`default` when `:disabled`) |
| Icon button (`.lc-iconbutton`) | `shell.css:241-274`, `shell.css:17680-17699` | `:hover:not(:disabled)` -> `background: --lc-hover-surface-strong`; `border-color: transparent`; `color: --lc-text-primary` | `background 140ms ease, color 140ms ease` | `pointer` (`default` when `:disabled`) |
| Segmented control button (`.lc-segmented > .lc-button`) | `shell.css:3635-3649` | Inactive `:hover:not(:disabled)` -> `background: --lc-hover-surface; color: --lc-text-secondary`; active keeps `--lc-segment-thumb` + `--lc-segment-thumb-shadow`; `:focus-visible` -> `outline-offset: 0` | `background/color/box-shadow 140ms ease` | `pointer` |
| Toggle switch (`button.lc-switch`) | `shell.css:3790-3795`, `shell.css:18793-18819` | Off `:hover:not(:disabled):not(.is-on)` -> `background: --lc-switch-off-hover` (`rgba(255,255,255,0.22)` / `rgba(28,30,29,0.22)`) | none | `pointer` (`default` when `:disabled`) |
| Pill chip (`.lc-chip`) | `shell.css:16031-16047` | `:hover` -> `background: --lc-bg-selected` | `background 140ms ease` | `pointer` |
| Link button (`.lc-linkbutton`) | `shell.css:4027-4037` | Base underlined in `--lc-lime`; inside `.lc-settings__pane` `:hover` -> `color: --lc-text-primary; text-decoration: underline; text-underline-offset: 2px` (`shell.css:25805-25812`) | none | `pointer` |
| Tab (`.lc-tab`) | `shell.css:9379-9389` | No `:hover` rule (base `cursor: pointer`, global `:focus-visible`) | none | `pointer` |
| Context menu row (`.lc-context__item`) | `shell.css:15909-15976` | `:hover:not(:disabled)` / `:focus-visible` / `.is-open` -> `background: --lc-bg-selected; outline: none`; `.is-danger:hover:not(:disabled)` / `:focus-visible` -> `background: --lc-red-tint` | none | `pointer` (`default` when `:disabled`) |
| Dropdown menu row (`.lc-menu__item`) | `shell.css:13400-13430` | `:hover` -> `background: --lc-hover-surface-strong` (`:disabled:hover` -> `transparent`) | `background 140ms ease` | `pointer` (`default` when `:disabled`) |
| Command palette row (`.lc-palette__item`) | `shell.css:13335-13354` | `:hover` / `.is-active` -> `background: --lc-hover-surface-strong` | `background 140ms ease` | `pointer` |
| Select button (`.lc-select__button`) | `shell.css:13463-13484` | `:focus-visible` -> `outline: none; border-color: --lc-focus-edge; box-shadow: 0 0 0 3px --lc-focus-glow` | none | `pointer` (`default` when `:disabled`) |
| Dialog / Inspector close button (`.lc-dialog__close`, `.lc-inspector__close`) | `shell.css:10745-10779`, `shell.css:9343-9371` | `:hover` -> `background: --lc-hover-surface-strong; color: --lc-text-primary` | `background 140ms ease, color 140ms ease` | `pointer` |
| Status chip & menu (`.lc-status.is-button`, `.lc-statusmenu__item`) | `shell.css:8268-8304` | `.lc-status.is-button:hover` / `:focus-visible` -> `filter: brightness(1.15); outline: none`; menu item `:hover` / `:focus-visible` -> `background: --lc-bg-control` | none | `pointer` |
| Text input (`.lc-input`) | `shell.css:10844-10862` | `:focus-visible` -> `outline: none; border-color: --lc-focus-edge; box-shadow: 0 0 0 3px --lc-focus-glow` | none | text |

### 3.4 Composer & Composer Foot Chips

| Element / Selector | Citation | Hover / Focus-Visible Change | Transition | Cursor |
|---|---|---|---|---|
| Composer box (`.lc-composer__box`) | `shell.css:8685-8712`, `shell.css:25503-25507` | `:hover` -> `box-shadow: 0 0 0 1px --lc-border-hairline`; in liquid look -> `box-shadow: inset 0 0 0 0.5px --lc-border-hover, inset 0 1px 0 --lc-liquid-rim-top, 0 10px 30px rgba(0,0,0,0.22)` | `box-shadow 140ms ease` | `text` |
| Base composer control (`.lc-control`, `.lc-control--boxed`, `.lc-control--bare`, `.lc-control.is-static`) | `shell.css:9088-9156`, `shell.css:8800-8805`, `shell.css:18243-18255` | `.lc-control:hover:not(:disabled)` -> `background: --lc-hover-surface`; `--boxed` -> `border-color: --lc-border-hover; background: --lc-hover-surface`; `--bare` -> `border-color: transparent; background: --lc-hover-surface-strong`; `.is-static:hover` -> `background: transparent` | none | `pointer` (`default` when `:disabled` or `.is-static`) |
| Composer foot chips (`.lc-composer__under` `.lc-control--boxed`, `.lc-chipbutton`, `.lc-control--folder`, `.lc-contextring__button`) | `shell.css:21859-21900`, `shell.css:25153-25161`, `shell.css:9172-9195` | `:hover:not(:disabled)` -> `background: --lc-chip-bg-hover` (`rgba(255,255,255,0.07)` / `rgba(28,30,29,0.07)`); `box-shadow: none`; `color: --lc-text-secondary`; `.lc-chipbutton:focus-visible` -> `outline: 2px solid --lc-lime-border-strong; outline-offset: 2px`; `.lc-contextring__button:focus-visible` -> `outline: 2px solid --lc-border-strong; outline-offset: 1px` | none | `pointer` (`.lc-contextring` base `default`, inside button `pointer`) |
| Send / Queue / Stop disc (`.lc-send`, `.is-queue`, `.is-stop`) | `shell.css:8813-8896`, `shell.css:22145-22168`, `MetalSend.tsx:330` | Base `.lc-send:not(:disabled):hover` -> `border-color: --lc-circle-border; background: --lc-circle-bg; color: --lc-circle-fg` (plus WebGL pointer halo in `MetalSend.tsx:330`); `.is-queue` -> `background: --lc-lime-tint-strong; border-color: --lc-lime-border-strong; color: --lc-lime-text`; `.is-stop` -> `background: --lc-red-tint; box-shadow: inset 0 0 0 1px --lc-red-border; color: --lc-red-text` | `background/border-color/color 140ms ease` | `pointer` (`default` when `:disabled`) |
| Plus menu button, satellites & labels (`.lc-plusmenu__button`, `__satellite`, `__label`) | `shell.css:21977-22014`, `shell.css:22057-22101` | `__button:focus-visible` -> `outline: 2px solid --lc-focus-ring; outline-offset: 2px`; `.is-open __satellite:hover:not(:disabled)` -> `background: --lc-hover-surface`; `__label:hover` -> `color: --lc-text-primary` | `opacity 120ms ease` on label | `pointer` (`default` when `:disabled`) |
| Route / model picker rows, fold, effort, blind (`.lc-picker__*`) | `shell.css:14098-14422`, `shell.css:16871-16875` | `__row:hover:not(:disabled)` -> `background: --lc-bg-selected` (`.is-recent` -> `--lc-hover-surface`); `__fold:hover` -> `background: --lc-bg-selected; color: --lc-text-primary` (`:focus-visible` -> `2px solid --lc-focus-ring`); `__effort:hover` -> `color: --lc-text-primary; border-color: --lc-border-hover`; `__blind:hover`/`:focus-visible` -> `color: --lc-text-primary; background: --lc-hover-surface`; `__input:focus-visible` -> `outline: none` | `140ms ease` on row & fold | `pointer` (`default` when `:disabled`) |
| Effort slider panel (`.lc-effortpanel__*`) | `shell.css:18327-18502` | `__fast:hover` -> `border-color: --lc-border-hover`; `:has(.lc-effortpanel__slider:focus-visible) .lc-effortpanel__track` -> `box-shadow: 0 0 0 2px --lc-focus-ring` | none | `__scale`: `pointer` (`.is-held`: `grabbing`, `:disabled`: `default`); `__what`: `help`; `__fast`: `pointer` |
| Attached / tagged / issue / reference chips (`.lc-tagged__chip`, `.lc-attached__tile`, `.lc-attachchip`, `.lc-issuechip`, `.lc-reference button`) | `shell.css:13795-13842`, `shell.css:18788-18793`, `shell.css:24953-25012` | `.lc-tagged__chip:hover` -> `border-color: --lc-lime-border-strong`; `.lc-attached__tile:hover` -> `color: --lc-text-primary; border-color: --lc-border-strong`; `.lc-attachchip:hover` -> `border-color: --lc-border-hover; color: --lc-text-primary`; `.lc-issuechip:hover` -> `background: --lc-hover-surface; color: --lc-text-primary` | `140ms ease` on attached tile | `pointer` |
| Queued message controls (`.lc-queued__check`, `.lc-queued__action`) | `shell.css:17908-17953` | `__action:hover:not(:disabled)` / `:focus-visible` -> `color: --lc-text-primary; text-decoration: underline; text-underline-offset: 3px` | none | `pointer` (`default` when `:disabled`) |
| Hover agent card on model chip (`.lc-control__anchor`) | `shell.css:6839-6844` | `:hover > .lc-agentcard--chip` / `:focus-within` -> `display: grid` | none | inherits |
| Slash menu item (`.lc-slash__item`) | `shell.css:13971-13984` | Active index tracked via `onMouseEnter` (`Composer.tsx:2111`) | none | `pointer` |

### 3.5 Conversation Thread & Links in Replies

| Element / Selector | Citation | Hover / Focus-Visible Change | Transition | Cursor |
|---|---|---|---|---|
| **Inline links in replies** (`.lc-link`) | `shell.css:16362-16391` | `:hover`, `:focus-visible` -> `border-bottom-color: var(--lc-blue-text)` (`#9dc2fb` / `#1f58b0`) | none | `pointer` (`.lc-linklabel`: `default`, `shell.css:16348-16362`) |
| User bubble edit pencil (`.lc-bubble:hover .lc-bubble__edit`, `.lc-bubble__edit`) | `shell.css:965-1007` | Bubble `:hover` / `:focus-visible` -> `opacity: 1`; button `:hover` -> `color: --lc-text-primary`; `:hover > svg` -> `background: --lc-hover-surface`; `:focus-visible > svg` -> `outline: 2px solid --lc-border-strong` | `opacity 140ms, color 140ms` | `pointer` |
| Reply copy button (`.lc-agentline__copy`) | `shell.css:23081-23115` | `.lc-agentline:hover` / `.lc-turnfoot:hover` / `:focus-visible` -> `opacity: 1`; `:hover` -> `color: --lc-text-primary; background: --lc-bg-control`; `:focus-visible` -> `outline: 2px solid --lc-focus-ring; outline-offset: 1px` | `opacity/color/background 140ms ease` | `pointer` |
| Jump-to-bottom pill (`.lc-jumpdown`, `.lc-jumpdown--working`) | `shell.css:19111-19186` | `:hover` -> `border-color: --lc-border-hover; color: --lc-text-primary`; while working, `:hover`/`:focus-visible` hides `.lc-jumpdown__dots` (`display: none`) and shows `.lc-jumpdown__chevron` (`display: flex`) | none | `pointer` |
| Prompt rail dots (`.lc-promptrail__mark`) | `shell.css:783-820` | `:hover::before` / `:focus-visible::before` -> grows to `8px × 8px`, `background: --lc-text-primary` | none | `pointer` |
| Thread version link & earlier button (`.lc-thread__versionlink`, `.lc-thread__earlier`) | `shell.css:1007-1046` | `__versionlink:hover`/`:focus-visible` -> `color: --lc-text-primary; text-decoration-color: currentColor`; `__earlier:hover`/`:focus-visible` -> `color: --lc-text-primary; border-color: --lc-border-strong` | none | `pointer` |
| Steps fold line & activity rows (`.lc-steps__line`, `.lc-activity`, `.lc-activity__more`) | `shell.css:1956-1976`, `shell.css:2016-2048`, `shell.css:13717-13752` | `.lc-steps__line:hover` -> `color: --lc-text-secondary` (`.is-static`: `default`, `--lc-text-muted`; `:focus-visible`: `2px solid --lc-focus-ring`); `.lc-activity:hover` -> `background: --lc-hover-surface`; `__more:hover` -> `background: --lc-hover-surface; color: --lc-text-secondary` | none | `pointer` (`default` when `.is-static`) |
| File rows & diff actions (`.lc-filerow`, `.lc-filerow__reveal`/view/copy, `.lc-diff__copy`, `.lc-diff__fold`/expand, `.lc-diff__noteadd`) | `shell.css:2120-2222`, `shell.css:2505-2536`, `shell.css:2656-2677` | `.lc-filerow:not(.is-static):hover` / `__line:hover` -> `background: --lc-hover-surface`; action icons `:hover` -> `background: --lc-hover-surface; color: --lc-text-secondary` (`:focus-visible`: `outline-offset: -2px`); `.lc-diff__row:hover .lc-diff__noteadd` -> `opacity: 1` (`140ms ease`) | `opacity 140ms ease` on noteadd | `pointer` (`default` when `.is-static`) |
| Shell output controls (`.lc-shellout__more`, `.lc-shellout__copy`) | `shell.css:13586-13641` | `__more:hover` -> `color: --lc-text-primary; background: --lc-bg-raised`; `__copy:hover` -> `color: --lc-text-primary`; `:focus-visible` -> `outline: 2px solid --lc-green; outline-offset: 2px` | none | `pointer` |
| Receipt & CLI artifacts summaries (`.lc-receipt__*`, `.lc-cliartifacts__summary`, `.lc-checkfixline__sent summary`) | `shell.css:2753-2815`, `shell.css:2926-2960`, `shell.css:10535-10545` | `__summary:hover` -> `color: --lc-text-primary` (or `--lc-text-secondary`); `__details:hover` -> `color: --lc-text-secondary; background: --lc-hover-surface` (`140ms ease`) | `140ms ease` on `__details` | `pointer` |
| Sent & handed files (`.lc-sentfile`, `.lc-handedfile__*`) | `shell.css:1134-1160`, `shell.css:1226-1283` | `.lc-sentfile:hover:not(:disabled)` -> `color: --lc-text-primary; border-color: --lc-border-strong`; `.lc-handedfile:hover` -> `border-color: --lc-border-strong`; `__open:hover` -> `color: --lc-text-primary`; `__save:hover` -> `color: --lc-text-primary; background: --lc-bg-control` | `140ms ease` | `pointer` (`default` when `:disabled` or `.is-elsewhere`) |
| Decision, question & diagnostic cards (`.lc-decision__option`, `.lc-question__option`, `.lc-diagnostic__action`, `.lc-rerun .lc-button`) | `shell.css:1733-1769`, `shell.css:18916-18946`, `shell.css:10626-10651`, `shell.css:16519-16544` | `.lc-decision__option:hover:not(:disabled)` -> `border-color: --lc-hover-border; background: --lc-chip-bg` (`140ms ease`); `.lc-question__option:hover` -> `border-color: --lc-border-strong; color: --lc-text-primary` (`:focus-visible`: `2px solid --lc-green`); `.lc-diagnostic__action:hover` -> `border-color: --lc-border-hover; background: --lc-hover-surface`; `.lc-rerun .lc-button:hover` -> `--lc-lime-tint-strong` + `--lc-lime-border-strong` | `140ms ease` on decision option | `pointer` (`default` when `:disabled`) |
| Live step peek & turn undo (`.lc-livestep__step.is-control`, `.lc-turnundo__*`) | `shell.css:10357-10403`, `shell.css:23426-23468` | `.lc-livestep__step.is-control:hover` -> `color: --lc-text-primary; border-color: --lc-border-hover`; `.lc-turnundo__ask`/cancel -> `background: --lc-bg-control; color: --lc-text-secondary`; `__go` -> `color: --lc-text-primary; outline: 1px solid --lc-border-strong` | none | `pointer` (`default` on peekcard/`:disabled`) |
| Peer handoff & call rows (`.lc-peer__*`, `.lc-callrow`, `.lc-calldivider__hide`) | `shell.css:15496-15525`, `shell.css:17953-17987`, `shell.css:19227-19254`, `shell.css:23715-23760` | `__toggle:hover` -> `--lc-hover-surface` + `--lc-text-secondary` (open: `--lc-chip-bg-hover`); `__bubble.is-link:hover`/`:focus-visible` -> `border-color: --lc-hover-border; background: --lc-hover-surface-strong` (`120ms ease`); `__open:hover` -> `--lc-text-primary`; `__origin:hover` -> `--lc-text-secondary`; `.lc-callrow:hover` -> `border-color: --lc-hover-border` | `140ms` / `120ms ease` | `pointer` |
| Selection-ask, thread find, doc preview, reply page, PDF/image tiles | `shell.css:22498-22519`, `shell.css:7067-7079`, `shell.css:2431-2478`, `shell.css:23153-23187`, `shell.css:24406-24440`, `shell.css:23402-23403` | `.lc-selectionask:hover` -> `--lc-hover-surface-strong`; `.lc-threadfind__step:hover` -> `--lc-bg-control` + `--lc-text-primary`; `.lc-docpreview__open:hover` -> `--lc-border-hover` + `--lc-hover-surface`; `__switch:hover` -> `--lc-text-secondary` + underline; `.lc-replypage__tab`/action `:hover` -> `--lc-text-primary`; `.lc-pdftile:hover` -> `--lc-border-strong` + `--lc-hover-surface-strong` (`140ms ease`) | `140ms ease` on pdftile | `pointer` |

### 3.6 Home Screen

| Element / Selector | Citation | Hover / Focus-Visible Change | Transition | Cursor |
|---|---|---|---|---|
| Teammate card (`.lc-hometeam__card`, `.lc-hometeam__go`) | `shell.css:7337-7354`, `shell.css:7392-7402`, `shell.css:7586-7592` | `:hover` -> `border-color: --lc-hover-border`; `background: linear-gradient(--lc-hover-surface, --lc-hover-surface), --lc-bg-raised`; reveals `.lc-hometeam__go` (`opacity: 1; transform: none`); `:focus-visible` -> `outline: 2px solid --lc-lime-border-strong; outline-offset: 2px` | `border-color 140ms ease, background 140ms ease` | `pointer` |
| Teammate card "Look now" eye button (`.lc-hometeam__look`) | `shell.css:7360-7392` | `:hover:not(:disabled)` -> `background: --lc-hover-surface; color: --lc-text-primary`; `:focus-visible` -> `outline: 2px solid --lc-lime-border-strong; outline-offset: 1px` | `background 140ms ease, color 140ms ease` | `pointer` (`default` when `:disabled`) |
| Starter prompt row (`.lc-homestart__start`, `.lc-homestart__compare`, `.lc-buildhead__compare`) | `shell.css:6561-6668`, `shell.css:25577-25582` | `__start:hover`/`:focus-visible` -> `background: --lc-hover-surface-strong` (`140ms ease`); row `:hover` reveals `__compare` (`opacity: 1` on fine pointer, `color: --lc-text-secondary; border-color: --lc-border-card`); `__compare:hover` -> `color: --lc-text-primary; border-color: --lc-border-strong; background: --lc-hover-surface-strong`; `.lc-buildhead__compare:hover` -> `color: --lc-text-primary` | `140ms ease` | `pointer` |
| Classic starter card (`.lc-starter`) | `shell.css:15354-15379` | `:hover:not(:disabled)` -> `background: --lc-bg-control; border-color: --lc-hover-border; color: --lc-text-primary` | `background 140ms ease, border-color 140ms ease` | `pointer` (`default` when `:disabled`) |
| While-you-were-away row (`.lc-away__row`) | `shell.css:7178-7204` | `:hover`, `:focus-visible` -> `background: --lc-bg-selected; outline: none` | none | `pointer` (`default` when `:disabled`) |
| Agent marks & cards (`.lc-agentmark`, `.lc-agenthead__more`, `.lc-agentmore`) | `shell.css:6799-6831`, `shell.css:7027-7045`, `shell.css:6184-6209` | `.lc-agentmark:hover`/`:focus-visible` -> `background: --lc-hover-surface` + reveals `.lc-agentcard` (`display: grid`); `:focus-visible` -> `box-shadow: 0 0 0 1.5px --lc-focus-edge`; `.lc-agenthead__more:hover` -> `--lc-text-primary` + `--lc-hover-surface`; `.lc-agentmore:hover` -> `--lc-text-primary` | `140ms ease` | `pointer` on buttons |
| Agent runtime cells & install controls (`.lc-runtimecell__install`, `__again`, `__quiet`, `.lc-installline__copy`) | `shell.css:5818-5882`, `shell.css:7639-7663`, `shell.css:7754-7771` | `__install:hover` -> `border-color: --lc-border-hover; color: --lc-text-primary`; `__again:hover` -> `color: --lc-text-primary; text-decoration: underline`; `__quiet:hover` -> `color: --lc-text-name`; `.lc-installline__copy:hover` -> `color: --lc-text-primary; border-color: --lc-border-strong` | none | `pointer` (`default` when `:disabled`) |
| Published app card (`.lc-published__card`) | `shell.css:8323-8347` | `:hover` -> `border-color: --lc-border-strong`; `:focus-visible` -> `outline: 2px solid --lc-border-strong; outline-offset: 2px` | none | `pointer` |
| Usage line & chip (`.lc-usageline__open`, `.lc-usagechip`) | `shell.css:23851-23953` | `__open:hover`/`:focus-visible` -> `color: --lc-text-primary` (bars -> `--lc-text-muted`); `.lc-usagechip:hover`/`:focus-visible` -> `background: --lc-hover-surface; color: --lc-text-primary` (`140ms ease`); both `:focus-visible` -> `outline: none; box-shadow: 0 0 0 1.5px --lc-focus-edge` | `140ms ease` on chip | `pointer` |

### 3.7 Settings Screen

| Element / Selector | Citation | Hover / Focus-Visible Change | Transition | Cursor |
|---|---|---|---|---|
| Settings nav tabs (`.lc-settings__navitem`) | `shell.css:5940-5960`, `shell.css:6001-6006` | `:hover` -> `background: --lc-bg-raised-2` (`#1b1e1f` / `#fcfbf9`); `color: --lc-text-primary`; `.lc-settings__navicon` -> `color: --lc-text-primary` | none | `pointer` |
| Settings search input (`.lc-settings__search`) | `shell.css:5935-5940` | `:focus-visible` -> `outline: 2px solid --lc-border-hover; outline-offset: 1px` | none | text |
| Settings row (`.lc-settingrow`) | `shell.css:12595-12606`, `shell.css:12968-12972` | Standalone `.lc-settingrow:hover` -> `background: --lc-hover-surface`; overridden inside `.lc-settings__pane .lc-settingrow:hover` and `.lc-remoteoutput:hover` -> `background: transparent` (rows in the Settings pane intentionally do not light up; their controls do) | none | default |
| Settings collapsible folds (`.lc-settings__fold > summary`, `.lc-settings__more > summary`, `.lc-stops__summary`) | `shell.css:13180-13214`, `shell.css:12439-12461`, `shell.css:12500-12518` | `__fold > summary:hover` -> `color: --lc-text-primary` (`:focus-visible`: `2px solid --lc-focus-ring`); `__more > summary:hover` -> `color: --lc-text-secondary`; `.lc-stops__summary:hover`/`:focus-visible` -> `background: --lc-hover-surface` | none | `pointer` |
| Settings inline links (`.lc-settings__pane .lc-linkbutton`, `.lc-settings__pane a`) | `shell.css:25805-25812` | `:hover` -> `color: --lc-text-primary; text-decoration: underline; text-underline-offset: 2px` | none | `pointer` |
| Face legend, look picker, hues, pets, roles (`.lc-facelegend`, `.lc-look`, `.lc-lookface button`, `.lc-hue`, `.lc-pets__action`, `.lc-pettile`, `.lc-rolecard`) | `shell.css:13089-13116`, `shell.css:10867-11152` | `.lc-look:hover` -> `--lc-hover-surface` (`140ms ease`); `.lc-lookface button:hover` -> `--lc-hover-surface; --lc-text-secondary` (`140ms ease`); `.lc-pets__action:hover` -> `--lc-text-primary` + underline; `.lc-pettile.is-removed:hover` -> `opacity: 0.85`; `.lc-rolecard:hover` -> `border-color: --lc-hover-border` (`140ms ease`) | `140ms ease` on look/rolecard | `pointer` (`.lc-pettile.is-busy`: `progress`, `:disabled`: `default`) |
| Connectors, MCP copy, Skills library, Memory switch | `shell.css:8521-8542`, `shell.css:16175-16189`, `shell.css:12907-12928`, `shell.css:12816-12833`, `shell.css:25309-25327`, `shell.css:3818-3843` | `.lc-addconnector__agent:hover` -> `--lc-hover-surface` + `--lc-hover-border` (`140ms ease`); `.lc-mcp-setup__copy:hover` -> `--lc-hover-surface` + `--lc-text-secondary` | `140ms ease` on addconnector | `pointer` (`.lc-skilllib__pack:disabled`: `wait`; `.is-held`: `default`) |

### 3.8 The Board, Missions & Team Board

| Element / Selector | Citation | Hover / Focus-Visible Change | Transition | Cursor |
|---|---|---|---|---|
| Board task card hover reveals (`.lc-task`) | `shell.css:3252-3258`, `shell.css:4818-4825` | `.lc-task:hover` / `:focus-within` -> reveals `.lc-task__more`, `.lc-task__actions`, and `.lc-ghostbutton` (`opacity: 1`) | `opacity 120ms ease` / `140ms ease` | `pointer` on actions |
| Board add-card button & link (`.lc-board__add`, `.lc-board__addlink`, `.lc-boardwait__open`) | `shell.css:4893-4898`, `shell.css:19259-19281`, `shell.css:12067-12084` | `.lc-board__add:hover`/`:focus-within` -> `opacity: 1` (`120ms ease`); `.lc-board__addlink:hover` -> `color: --lc-text-primary` | `opacity 120ms ease` | `pointer` |
| Board conversation card (`.lc-convcard`) | `shell.css:11970-11995` | `:hover`, `:focus-visible` -> `border-color: --lc-border-strong` | none | `pointer` |
| Missions list row & multi-pick (`.lc-missionrow`, `.lc-missionrowwrap`, `.lc-missionrow__pick`, `.lc-pickbar__*`, `.lc-filter`) | `shell.css:11644-11658`, `shell.css:11727-11747`, `shell.css:20296-20377`, `shell.css:26223-26230` | `.lc-missionrow:hover` / `.lc-missionrowwrap:hover` -> `background: --lc-hover-surface` (`140ms ease`); wrap `:hover` reveals `.lc-missionrow__pick` (`opacity: 1` on fine pointer); `.lc-filter:hover` -> `background: --lc-bg-raised-2`; `.lc-pickbar__link:hover` -> `color: --lc-text-secondary`; `.lc-pickbar__delete:hover` -> `background: --lc-red-tint` | `140ms ease` | `pointer` (`__pick:disabled`: `not-allowed`) |
| Team board roster cards (`.lc-rostercard__*`, `.lc-teammate__mission*`) | `shell.css:11372-11433`, `shell.css:11904-11909`, `shell.css:12148-12225`, `shell.css:15797-15845`, `shell.css:16633-16739` | `.lc-rostercard:hover`/`:focus-within` -> reveals `__actions` (`opacity: 1`); `--new:hover` -> `--lc-hover-border` + `--lc-hover-surface`; `__message:hover` / `__edit:hover` -> `--lc-hover-surface-strong; border-color: transparent; --lc-text-primary`; `__remove:hover` -> `--lc-red-tint; --lc-red-text`; `__mission:hover` -> `--lc-hover-surface`; `__waiting:hover __answer` -> `text-decoration: underline`; `__missionmenu:hover` -> `--lc-bg-control; --lc-text-secondary` | `opacity 140ms ease` on missionmenu | `pointer` |

### 3.9 Rooms Screen

| Element / Selector | Citation | Hover / Focus-Visible Change | Transition | Cursor |
|---|---|---|---|---|
| Room card (`.lc-roomcard`) | `shell.css:4077-4094` | `:hover` -> `border-color: --lc-border-hover` | none | `pointer` |
| Room title rename button (`.lc-room__name`) | `shell.css:3303-3326` | `:hover:not(:disabled)` -> `text-decoration: underline` | none | `pointer` (`default` when `:disabled`) |
| Room roster / exchange folds (`.lc-roster__summary`, `.lc-exchange__rest`, `.lc-exchange__fold`) | `shell.css:3395-3418`, `shell.css:3487-3531` | `.lc-roster__summary:hover` -> `color: --lc-text-secondary`; `.lc-exchange__rest:hover` -> `color: --lc-text-primary` | none | `pointer` |
| Post header face button (`.lc-posthead__face`) | `shell.css:4548-4563` | `:hover` -> `opacity: 1` | none | `pointer` |
| Continuation part pill (`.lc-continuation__part`, `__putback`) | `shell.css:4934-4974` | `__part:hover`/`:focus-visible` -> `border-color: --lc-border-hover; color: --lc-text-primary` | none | `pointer` |

### 3.10 Routines Screen

| Element / Selector | Citation | Hover / Focus-Visible Change | Transition | Cursor |
|---|---|---|---|---|
| Add routine card (`.lc-routineadd`) | `shell.css:17650-17669` | `:hover`, `:focus-visible` -> `background: --lc-bg-raised-2` | none | `pointer` |
| Remove routine icon button (`.lc-automations .lc-routinerow__remove`) | `shell.css:17699-17712` | `:hover`, `:focus-visible` -> `background: --lc-red-tint; border-color: transparent; color: --lc-red-text` | inherits `140ms ease` | `pointer` |
| Routine step checkbox / save routine (`.lc-routinestep__check`, `.lc-saveroutine`) | `shell.css:17261-17270`, `shell.css:11304-11309` | `.lc-saveroutine:hover` -> `color: --lc-text-secondary; border-color: --lc-hover-border` | none | `pointer` |
| Templates & savable rows (`.lc-templates__row`, `.lc-savable__row`, `.lc-savable__name`, `.lc-recovery__details > summary`) | `shell.css:3084-3164`, `shell.css:17800-17805` | `.lc-templates__row:hover`/`:focus-visible` -> `background: --lc-hover-surface`; `.lc-savable__row:hover` -> `background: --lc-hover-surface`; `.lc-savable__name:hover` -> `text-decoration: underline` | none | `pointer` |

### 3.11 Side Panels (Changes, File Viewer, Terminal, Browser, Compare/Benchmarks)

| Element / Selector | Citation | Hover / Focus-Visible Change | Transition | Cursor |
|---|---|---|---|---|
| Changes tree, commits, walk, menu, commit link (`.lc-changes__*`, `.lc-commit__link`, `.lc-review__turn`) | `shell.css:7939-8092`, `shell.css:8170-8188`, `shell.css:21018-21038`, `shell.css:23578-23643` | `__menuitem:hover`/`:focus-visible` -> `--lc-bg-control; --lc-text-primary`; `__treefolder`/`__treefile`/`__commit`/`.lc-review__turn` `:hover` -> `--lc-hover-surface`; `__walk:hover` -> `--lc-text-primary; --lc-bg-control` (`140ms ease`, `:focus-visible`: `2px solid --lc-text-secondary`) | `140ms ease` on walk | `pointer` (`default` when `:disabled`) |
| File viewer, sheet tabs, zoomable image (`.lc-viewer__*`, `.lc-sheet__tab`, `.lc-zoomimage__*`) | `shell.css:21171-21190`, `shell.css:21372-21399`, `shell.css:21510-21541`, `shell.css:21623-21673` | `__action`/`__close` `:hover` -> `--lc-text-primary; --lc-bg-control` (`140ms ease`); `.lc-sheet__tab:hover` -> `--lc-border-hover; --lc-hover-surface`; `__version:hover` -> `--lc-hover-surface; --lc-text-secondary`; `.lc-zoomimage__percent`/actual `:hover`/`:focus-visible` -> `--lc-bg-control; --lc-text-primary` | `140ms ease` on viewer action/close | `pointer` (`.lc-zoomimage__pane.is-pannable`: `grab` / `:active`: `grabbing`) |
| Terminal & Browser tabs/menus/point (`.lc-terminal__*`, `.lc-browser__*`, `.lc-pagepoint__remove`) | `shell.css:7939-7958`, `shell.css:24663-24688`, `shell.css:24901-24921` | `__newtabmenu button:hover`/`:focus-visible` -> `background: --lc-bg-selected`; `.lc-browser__point:hover` / `.lc-pagepoint__remove:hover` -> `--lc-text-primary; --lc-bg-control` (`140ms ease`) | `140ms ease` on point | `pointer` (`default` when `:disabled`) |
| Compare & Benchmarks (`.lc-benchmark__run`, `.lc-benchmarks__row`, `.lc-compare__*`, `.lc-compared__open`) | `shell.css:14842-14941`, `shell.css:15175-15187`, `shell.css:17520-17550` | `__run:hover` -> `--lc-border-strong; --lc-text-primary` (`140ms ease`); `__row:hover`/`:focus-visible` -> `--lc-bg-raised-2` + chevron `--lc-text-primary`; `.lc-compare__tool:hover`/`:focus-visible` -> `--lc-text-primary; --lc-hover-surface`; `.lc-compare__head.is-rail:hover __name` -> `--lc-text-primary` | `140ms ease` on run | `pointer` (`default` when `:disabled`) |
| Section fold & error reload (`.lc-sectionlabel--fold`, `.lc-windowfailed__reload`) | `shell.css:17987-18012`, `shell.css:23045-23066` | `__fold:hover` -> `color: --lc-text-secondary`; `__reload:hover` -> `border-color: --lc-border-hover; color: --lc-text-primary` | none | `pointer` |

### 3.12 TSX Pointer/Hover Handlers (`components/*.tsx`)
No inline `style={{ cursor: ... }}` exists in `components/*.tsx` (`NOT FOUND`). Components use React pointer/mouse events only for stateful hover behaviors:
- Active-row tracking in keyboard-navigable lists: `CommandPalette.tsx:160`, `Select.tsx:291`, `ContextMenu.tsx:347-351`, `Composer.tsx:2054,2075,2111` (`@` and `/` menus), `RoomScreen.tsx:1588` (mentions), `IssueList.tsx:76`.
- Hover flyouts / popovers / shader / bot glance: `Sidebar.tsx:1257-1258,1478-1479,1910` + `RailFlyout.tsx:121-122` (rail flyout), `PromptRail.tsx:211` (turn tooltip), `ThreadItems.tsx:985-989` (step peek), `TeammateBot.tsx:443` (face notices pointer), `MetalSend.tsx:330` (WebGL cursor-bend halo).

---

## 4. Rust App & Rune Audit (`apps/native` & `vendor/rune`)

### 4.1 What Rune Has (and Does Not Have)

**Hover support in Rune — YES, paint-only and rich:**
- `El::hover(c: Color)` (`vendor/rune/crates/rune/src/el/visuals.rs:287-290`): draws an overlay color over the element's fill while hovered. Default timing: instant enter, `0.1 s` fade-out (`el/visuals.rs:464-465`).
- `El::hover_motion(enter, exit)` and `El::hover_motion_within(enter, exit)` (`el/visuals.rs:455-470`): configurable enter/exit curves (e.g., `140ms` ease) inherited down the tree.
- `El::hover_group()`, `El::hover_group_shared(key)`, `El::hover_group_focus()`, `El::hover_alone()` (`el/visuals.rs:433-492`) + `El::group_opacity(rest, hovered)` (`el/visuals.rs:476-479`) + `El::group_color(c)` (`el/visuals.rs:481-484`): parent-hover reveal and text/icon color changes without state commits.
- `El::hover_delays(enter, leave)` (`el/layout.rs:396`) + `El::on_hover_change` (`runtime.rs:624-628`): delayed hover callbacks, used by Locust's tooltip helper (`apps/native/src/ui/hover.rs:30-49`).
- `rune::Link::hover(paint: LinkPaint)` (`vendor/rune/crates/rune/src/text/span.rs:248`): color and underline on hovered inline text links.
- **Missing from Rune (`NOT FOUND`):** `hover_border` / `group_border`, `hover_shadow`, `hover_filter`, and non-`Link` `hover_underline` do not exist on `El`. Changing a border on hover currently forces stateful `.on_hover_change` (as `ui/jump_to_bottom.rs:68-70` does).

**Cursor support in Rune — YES, full OS cursor pipeline, but opt-in:**
- `pub enum Cursor` (`vendor/rune/crates/rune/src/cursor.rs:15-37`): `Default`, `Pointer`, `Text`, `Grab`, `Grabbing`, `NotAllowed`, `Move`, `ResizeColumn`, `ResizeRow`, `Crosshair`, `Hidden`, `Image(CursorImage)`. (`Help`, `Wait`, `Progress` are `NOT FOUND`.)
- `El::cursor(cursor: Cursor)` (`vendor/rune/crates/rune/src/el/layout.rs:271`).
- `Runtime::cursor(&self) -> Cursor` (`vendor/rune/crates/rune/src/runtime/input/pointer.rs:364-390`):
  - Returns `Cursor::Text` automatically for inputs / `role == "textbox"` (`pointer.rs:377-379`).
  - Returns `Cursor::Pointer` automatically over inline `Link` spans (`pointer.rs:380-382`, `el/text.rs:343`).
  - Walks up ancestors for an explicit `handlers.cursor`, otherwise returns `Cursor::Default` (`pointer.rs:383-389`). **`.on_press(...)` and `role("button")` do NOT set `Cursor::Pointer` automatically.**
- `OsCursor::apply` (`vendor/rune/crates/rune/src/cursor/os.rs:48-65, 168-181`): maps `Cursor::Pointer => CursorIcon::Pointer` and calls `window.set_cursor(icon(other))` via `winit` (through `platform::macos::cursor::window_cursor` on macOS, `cursor/os.rs:55-56`).

### 4.2 What `apps/native/src` Actually Wires

- **Cursor (`Cursor::Pointer`): `NOT FOUND` in `apps/native/src`.**
  - There is exactly **1** `.cursor(...)` call in all of `apps/native/src`: `apps/native/src/ui/terminal_panel.rs:586` (`.cursor(rune::Cursor::ResizeColumn)`).
  - **0** calls to `.cursor(Cursor::Pointer)` exist in `apps/native/src` — not in `look/controls.rs` (`button`, `primary_button`, `ghost_button`, `deny_button`, `chip`, `switch`, `composer_chip`, `send_circle`, `header_button`), not in `ui/sidebar.rs`, `ui/home_panel.rs`, `ui/settings_panel.rs`, `ui/routines_panel.rs`, `ui/room_screen.rs`, or `ui/context_menu.rs`.
  - `rune-kit` itself also does NOT set `Cursor::Pointer` on `Button`, `IconButton`, `Select`, `Segmented`, `Switch`, or `List` (`rune-kit` only sets `Cursor::Pointer` on diff actions, sortable table headers, and markdown task checkboxes).
  - Result: the hand pointer **never shows** on any Locust button, row, tab, chip, menu item, or card — only on inline reply links (`ui/reply_actions.rs:721`).
- **Hover coverage: 62 `.hover(...)` calls vs. 483 production `.on_press` / `.on_press_at` handlers (12.8%).**
  - `look/controls.rs` wires `.hover(...)` on `button` (`L62`), `primary_button` (`L79`), `chip` (`L95`), `composer_route_chip` (`L312`), `send_circle` (`L373,L377`), `ghost_button` (`L409`), `deny_button` (`L423`), `header_button` (`L439`). **`controls::switch` (`L127-150`) has NO `.hover(...)`.**
  - `look/mod.rs:254-255` maps `Kit.c.hover = c("hover-surface")` and `Kit.c.pressed = c("hover-surface-strong")`, so `rune_kit::IconButton` (`controls::head_icon`), `Select`, and `Segmented` get a `--lc-hover-surface` wash.
  - Major screens with massive hover gaps:
    - `ui/home_panel.rs`: 27 `.on_press`, only 3 `.hover(...)` (`L1230,L1588,L1680`). Teammate cards (`L1580-1583`) have **no hover**.
    - `ui/settings_panel.rs`: 41 `.on_press`, only 2 `.hover(...)` (`L458` Copy, `L1104` face legend). Settings nav tabs have **no hover**.
    - `ui/routines_panel.rs`: 51 `.on_press`, only 1 `.hover(...)` (`L937`).
    - `ui/people_panel.rs`: 43 `.on_press`, **0** `.hover(...)`.
    - `ui/chat_apps_section.rs` (21 `.on_press`), `ui/scout_setting.rs` (18), `ui/browser_panel.rs` (13), `ui/claude_cloud_panel.rs` (10), `ui/activity_drawers.rs` (8), `ui/approval_card.rs` (7), `ui/background_runs.rs` (7), `ui/terminal_panel.rs` (7): **0** `.hover(...)` each.
    - `ui/sidebar.rs`: 26 `.on_press`, 8 `.hover(l.c("hover-surface"))` (`L436,L946,L1256,L1400,L1611,L1640,L1650,L1675`).
  - `hover_group_focus()` + `group_opacity()` is used in only **1** file (`ui/jump_to_bottom.rs:48-49,66`). `group_color()`, `hover_group_shared()`, and `hover_motion_within()` are `NOT FOUND` in `apps/native/src`.

### 4.3 Electron's 14 Hover Looks vs. Rust Status

| # | Electron Hover Look | In Rust (`apps/native/src`)? | Citation / Notes |
|---|---|---|---|
| 1 | **Surface wash (`--lc-hover-surface`)** | **Partial (47 calls)** | Used in `look/controls.rs:62,312,377,409,439`, `ui/sidebar.rs` (8), `ui/rail.rs` (6), `ui/room_screen.rs` (5), `ui/home_panel.rs` (3), `ui/missions_screen.rs` (2), `look/mod.rs:254` (`rune-kit`). Missing on most `.on_press` controls (421/483 have no `.hover()`). |
| 2 | **Strong surface wash (`--lc-hover-surface-strong`)** | **NOT FOUND** | Token loaded in `look/mod.rs:255` only as `p.pressed`, never passed to `.hover(...)`. `controls::ghost_button` (`controls.rs:409`) uses `hover-surface` instead. |
| 3 | **Border lift + surface wash** | **NOT FOUND** | Rune has no `hover_border`. `controls::button` (`controls.rs:62`) and `ghost_button` (`controls.rs:409`) do fill wash only; Home teammate cards (`ui/home_panel.rs:1580`) have neither. |
| 4 | **Border lift only** | **Partial (1 stateful workaround)** | Only `ui/jump_to_bottom.rs:68-70` (via `s.jump_hover`). All other cards/pills/rows lack border hover. |
| 5 | **Elevated fill (`bg-raised-2` / `bg-selected` / `bg-control`)** | **Partial (10 `bg-selected`, 0 `bg-raised-2`/`bg-control`)** | `bg-selected` used in `controls.rs:95` (chip), `ui/context_menu.rs:349`, `ui/thread_view.rs:1151,1609,1724`, `ui/side_chat.rs:392,415`. `bg-raised-2` (Settings nav tabs, `.lc-row`) and `bg-control` are `NOT FOUND` in `.hover(...)`. |
| 6 | **Primary solid button brighten (`--lc-primary-bg-hover`)** | **Yes (1 call)** | `look/controls.rs:79` (`primary_button`). |
| 7 | **Composer frosted chip wash (`--lc-chip-bg-hover`)** | **NOT FOUND** | `look/controls.rs:312` (`composer_route_chip`) uses `hover-surface` instead of `chip-bg-hover`. |
| 8 | **Danger red tint (`--lc-red-tint`)** | **Partial (3 calls)** | `look/controls.rs:373` (`send_circle` Stop), `look/controls.rs:423` (`deny_button`), `ui/context_menu.rs:349` (danger item). Missing on routine/roster remove buttons and close chrome. |
| 9 | **Monochrome accent tint boost (`--lc-lime-tint-strong`)** | **NOT FOUND** | Not passed to `.hover(...)` anywhere (`send_circle` Ready uses `hover-surface`, `controls.rs:377`). |
| 10 | **Text ink brighten only** | **NOT FOUND** | Rune's `El::group_color` is never called in `apps/native/src` (`NOT FOUND`). |
| 11 | **Link underline / bottom-border underline** | **Partial (inline reply links only)** | `ui/reply_actions.rs:717-721` (`rune::Link::hover` with `LinkPaint::new().color(...).underline(...)`). Non-reply link buttons (`lc-linkbutton`, Settings links, room title) are `NOT FOUND`. |
| 12 | **Group-hover reveal / swap** | **Partial (1 control)** | Only `ui/jump_to_bottom.rs:48-49,66` (`hover_group_focus` + `group_opacity`). Missing on sidebar conversation row menus, bubble edit pencil, reply copy button, board task actions, home card arrow, and compare buttons. |
| 13 | **Switch track brighten (`--lc-switch-off-hover`)** | **NOT FOUND** | `look/controls.rs:127-150` (`controls::switch`) has no `.hover(...)`. |
| 14 | **Special / non-fill feedback** | **NOT FOUND** | No `.lc-composer__box` hover ring (`controls.rs:388-399`), no `brightness(1.15)` status chip hover, no `MetalSend` pointer halo. |
