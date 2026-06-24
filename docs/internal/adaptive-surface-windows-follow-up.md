# Adaptive Surface Windows Follow-up

## Problem

Windows currently derives sidebar panel entries from App UI activators, but the
derived panel identity follows the activator id, for example
`lingxia-chatSidebar`. The declared surface identity remains `lingxia-chat`.
This splits one logical surface into two identities:

- JS/API and App UI declarations refer to the surface id.
- Windows footer activators and panel layout refer to the activator-derived panel id.

macOS does not have this split. Its App UI runtime keeps `surfaceById` as the
source of truth. Activators store `action.surface`, and shell/sidebar callbacks
toggle or open that same managed surface id.

## Goal

Use the declared surface id as the stable cross-platform business key. Treat
activator ids as UI element identities only. Treat Windows panels as the Windows
presentation of an aside surface, not as a separate model.

## Proposed Direction

1. Add a Windows-side managed surface registry built from `ui.json`.
   - Key by `surface.id`.
   - Store related activator id, host surface, role, edge, content kind, app id,
     path, icon, label, and sizing metadata.
   - Keep `PanelsConfig` as legacy fallback only.

2. Update Windows sidebar/footer activators to dispatch the target `surface.id`.
   - Hit testing may still use activator id internally.
   - Behavior commands should resolve to the managed surface id before opening.

3. Update Windows managed-surface handlers to accept surface ids.
   - `setManagedSurfaceVisible("lingxia-chat", true)` should open the right
     aside panel.
   - Existing panel-id compatibility can remain as a temporary fallback.

4. Align demo/API usage with declared surfaces.
   - Prefer `lx.openSurface({ surface: "lingxia-chat" })` for the AI Chat entry.
   - Avoid making `navigateToLxApp({ appId })` implicitly guess aside behavior
     unless product semantics explicitly require that compatibility.

5. Later, extract the shared App UI parsing model.
   - Candidate home: `lingxia-shell` or a new small App UI model crate.
   - Avoid putting this registry in `lingxia-app-context`; it is UI runtime
     state/semantics, not general app context.

## Acceptance Checks

- Footer AI Chat activator and `lx.openSurface({ surface: "lingxia-chat" })`
  open the same Windows aside.
- The active/visible state is shared between API-opened and activator-opened
  surfaces.
- macOS and Windows both use surface id as the cross-platform contract.
- Legacy Windows panel config continues to work during migration.
