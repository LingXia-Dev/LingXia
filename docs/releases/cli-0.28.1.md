# LingXia 0.28.1

## Breaking changes

### one side panel for lxapps and browser tabs; two aside slots

Asides group into two slots, `panel` (lxapps and browser tabs in one region) and `native` (terminal), instead of `lxapp` / `browser` / `native`; an expanded window admits two. `SurfaceLayoutAsideSlot.kind` (lx automation) is now `'panel' | 'native'`: compare with `'panel'` where tests matched `'lxapp'` or `'browser'`. Rust: `SlotKind::Lxapp` and `SlotKind::Browser` are `SlotKind::Panel`; the Windows contract's `ASIDE_LXAPP_PANEL_ID` and `ASIDE_BROWSER_PANEL_ID` are `ASIDE_PANEL_ID`. Slot APIs still accept `"lxapp"` and `"browser"` as names for the panel slot.
