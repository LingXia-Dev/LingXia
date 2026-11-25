_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Error codes

Every message starts with its code: `CODE: detail`. **build** codes stop `lingxia build` with
`file:line:col`; **runtime** codes come from the View runtime with the JSX trace (`at <tag> (file:line:col)`);
**native** codes come from the host decoder, layout or renderer and keep the last good frame.

| Code | Raised at | Severity | When | Fix |
| --- | --- | --- | --- | --- |
| `UI_A11Y` | build, runtime, native | error | An image lacks alt, or a control, editor, plot, select, dialog or webview lacks an accessible name. | Add alt (or alt="" when decorative) on <image>, or aria-label / visible text on the control. |
| `UI_ACTION` | runtime, native | error | An action call was malformed, cyclic, unknown, or its reply was invalid. | Call a declared action from view.actions with at most one acyclic JSON payload. |
| `UI_ANIMATION` | build, runtime, native | error | An animation definition is invalid or over budget, or style.animation was set inline. | Use animate-* utilities with valid @keyframes in the theme stylesheet and keep definitions within animationBytes. |
| `UI_BACK` | runtime | error | useBackHandler was given an enabled flag that is not a boolean or an onBack that is not a function. | Pass a boolean enabled and an onBack function to useBackHandler. |
| `UI_BLEED_PARENT` | runtime | error | bleed was placed on an element that is not a direct child of <page>. | Move bleed to a direct child of <page>. |
| `UI_BOOTSTRAP_TIMEOUT` | native | recovery | Logic did not deliver the first page state in time. | Ensure the page Logic initializes and resolves its initial state promptly without blocking. |
| `UI_BRIDGE` | native | recovery | The View-Logic connection failed or was replaced. | Reload the page or verify the host bridge session is still active. |
| `UI_BUDGET` | runtime, native | warning | A frame used at least three quarters of a page limit, or a dialog or popover past topLayer stayed closed. | Reduce rendered nodes/overlays or virtualise long lists with <List>. |
| `UI_BUFFER` | native | error | A malformed wire buffer reached the native decoder. | Rebuild the page with a matching @lingxia/view compiler and host SDK. |
| `UI_BUTTON` | build, runtime, native | error | A button label is invalid, a scroll, list or webview sits inside a composed control, or a control wraps only another control. | Move scroll/list/webview outside pressable controls and put the action directly on the inner control. |
| `UI_CHANNEL` | runtime | error | A useChannel call, channel handler, payload or frame is invalid. | Pass a valid channel action name, acyclic JSON params and a valid onData handler. |
| `UI_CHECK` | runtime, native | error | A checkbox or switch value, defaultChecked, label or binding is invalid. | Bind a boolean useLocal cell or pass a boolean value with onChange. |
| `UI_CHILD` | runtime, native | error | An element has children it does not accept, or lacks its required parent. | Match the element child kind in elements.md (leaf elements take no children; runs belong inside <text>). |
| `UI_CHROME` | native | error | The platform adapter reported invalid host chrome geometry. | Check the host platform adapter chrome inset measurements. |
| `UI_CLASS` | build, runtime | error | A className is not a string, exceeds 16384 characters, or names a utility the build did not compile. At build, a warning where a className is built at run time (concatenation, a template hole of unknown values, page data); with strictClasses, at every className the build cannot follow to literals. | Write complete Tailwind utility names in literals, const tables, conditionals or cx() arguments. |
| `UI_CLASS_EDGE` | build | warning | A border or divide colour is a non-line role at an opacity (`border-text/12`), which high contrast does not strengthen. | Use `border-line` for a separator or `border-line-strong` for an interactive boundary; `disabled:` edges and a tone's edge on its own `-soft` fill (`border-accent/40 bg-accent-soft`) are exempt. |
| `UI_CLASS_ELEMENT` | build, runtime | error | A supported utility is used on an element that does not render it, or a state variant is used on an element that never enters it. | Move layout utilities to the enclosing container and apply state variants only to elements that support them. |
| `UI_CLASS_UNKNOWN` | build | error | A class token is not a Tailwind utility, or is one the build found in no string of the page's sources. | Replace it with the nearest supported utility named in the message, define a @utility in the theme stylesheet, or write the class as one whole word in a string literal. |
| `UI_CLASS_UNSUPPORTED` | build | error | A Tailwind utility lowers to CSS the native renderer cannot draw. | Use the supported alternative named in the diagnostic or in utilities.md. |
| `UI_COLOR` | build, runtime, native | error | A colour is not a role, currentColor or transparent, or a colour class uses dark:. | Use a theme colour role (e.g. bg-surface, text-muted, accent/12) and drop dark: on colours; hex is valid only in vectors, gradients and shadows. |
| `UI_COMMIT` | runtime | error | A commit was requested without a prepared candidate frame. | Call prepare before commit; do not commit a discarded render. |
| `UI_COMPAT` | runtime, native | recovery | The installed host is older than the compiled page contract. | Update the host app or embedded LingXia SDK to match the compiler version. |
| `UI_COMPOSITE` | native | error | A composited effect (backdrop or opacity group) is out of bounds. | Keep composited layers within the page composite bounds. |
| `UI_CONTEXT` | build, runtime | error | createContext is not a module-level const, Provider lacks a value or single element child, or the value is not plain data, functions or handles. | Declare createContext as a module-level const and pass plain data, functions or Local handles to <Ctx.Provider value={...}> wrapping one element. |
| `UI_CSS` | build | error | A stylesheet contains rules other than tokens, or a page imports more than one stylesheet. | Import at most one stylesheet per page containing only @theme, @utility, @font-face and :root/[data-theme] token blocks. |
| `UI_DOM` | build | error | A DOM global (window, document, localStorage, HTMLElement) is referenced in View code. | Remove DOM access; use view.env in the View or move platform/storage calls to Logic. |
| `UI_ENVIRONMENT` | native | error | The host reported an invalid View environment. | Ensure the host adapter supplies a valid viewport, theme, direction and language. |
| `UI_ERROR` | runtime | error | A failure occurred without a more specific UI_* code. | Inspect the error message and JSX stack trace to resolve the thrown exception. |
| `UI_FIELD` | native | error | A wire field reached a tag that does not accept it. | Rebuild the page with a matching compiler and host SDK contract. |
| `UI_FIELDS` | native | error | A malformed field list reached the native decoder. | Rebuild the page with a matching compiler and host SDK contract. |
| `UI_FILTER` | native | error | A backdrop filter is outside its allowed bounds. | Keep backdrop blur within 0..64px and saturation within 0..16. |
| `UI_FONT` | build, runtime, native | error | A font family is not declared with @font-face, its file is missing, outside the lxapp or too large, or a face could not be read. | Declare bundled .ttf/.otf/.ttc files (<= 8 MiB) inside the lxapp with @font-face and include a generic fallback. |
| `UI_FRAGMENT` | build, runtime | error | A page returns a fragment instead of a single root element. | Wrap the page output in a single <page> element. |
| `UI_FRAME` | native | warning | The platform presenter kept refusing frames prepared ahead and fell back to one frame at a time. | No authoring change required; inspect host presenter logs if frame pacing degrades. |
| `UI_GPU_LOST` | native | recovery | The GPU device failed mid-frame; after a second failure the page shows its recovery screen. | Reload the page; if persistent on one device, verify driver fallback behaviour. |
| `UI_GRADIENT` | runtime, native | error | A gradient has geometry its kind does not take, or invalid stops. | Use linear, radial or conic gradient syntax with 2..16 valid colour stops. |
| `UI_GRID` | native | error | An invalid grid track list or line placement reached layout. | Use supported CSS grid track functions (up to 64 tracks) and integer/span line placements. |
| `UI_HANDLER` | runtime, native | error | An event handler threw or its promise rejected (fails the page in dev/automation, logged in release). | Handle errors inside the event handler or use useAction().run() which returns { ok, error }. |
| `UI_I18N` | runtime | error | t() names a key no catalogue has, or the lxapp has no i18n/ catalogues. | Add the key to i18n/<source>.json and regenerate catalogues with lingxia check. |
| `UI_ID` | runtime, native | error | Two elements share an id, a popover lacks an id, an id has spaces, or popoverTarget names no popover. | Give each element a unique space-free id (use useId() inside reusable components) and match popoverTarget to an existing popover id. |
| `UI_IMAGE` | runtime, native | error | An image source, alt, retryKey or callback is invalid, or loading failed. | Pass a valid package path, lx:// URI or allowed http(s) URL, a string alt, and valid onLoad/onError functions. |
| `UI_INERT` | build | warning | A control has no handler, bind or disabled prop, so it is focusable but does nothing. | Add the control handler or bind prop, or mark the control disabled. |
| `UI_INPUT` | runtime, native | error | An editor prop, binding, type combination or text value is invalid. | Bind a string useLocal cell (or pass value + onInput) and use min/max/step only on number, date or time inputs. |
| `UI_KEY` | build, runtime, native | error | A sibling key is non-string or duplicated, or a component is keyed by its own function name. | Use unique non-empty string keys among siblings and never key by Component.name. |
| `UI_LAYOUT` | native | warning | A page-bottom wrapper's `<scroll>` or `<List>` shares the wrapper with other in-flow content, so it stops above the bottom bars instead of running on under them. | Make the scroll the wrapper's only in-flow child and position the rest absolute. |
| `UI_LEAVE` | runtime, native | error | A useLeaveGuard call, name, enabled option or answer is invalid. | Pass a unique name, boolean enabled flag, and answer with a boolean. |
| `UI_LIMIT` | runtime, native | error | A page-wide or payload limit was exceeded. | Reduce rendered nodes/depth/payload size or switch long lists to <List> (see limits.md). |
| `UI_LINK` | build, runtime, native | error | A <link> has no to, onClick, popoverTarget or interestFor, or a block link nests inside another. | Give <link> at least one of to, onClick, popoverTarget or interestFor, and do not nest block links. |
| `UI_LIST` | runtime, native | error | A <List> description, keys, anchor or row height is invalid. | Provide unique string keys within itemKeyBytes, valid anchor fields and positive finite row heights. |
| `UI_LOCAL` | runtime | error | Two useLocal calls share a name in one component, a non-JSON value is stored, or set/focus is called after unmount. | Use distinct names per component, store only plain JSON values, and guard calls after unmount. |
| `UI_LX` | build | error | lx or a value import from @lingxia/logic is used in View code. | Move lx.* calls into a Logic action and invoke that action from the View; use `import type` for types. |
| `UI_MODULE` | build, native | error | A page module failed to load, imported a reserved path, missed a peer dependency, or mismatched its dev chunk interface. | Install missing peer dependencies, import only source/package modules, and rebuild stale pages. |
| `UI_OVERRUN` | native | warning | A render exceeded its CPU time budget and was dropped to retry. | Reduce synchronous render work or move heavy computation to Logic. |
| `UI_PAGE` | native | error | A call reached a page that was already discarded. | Stop driving the closed page handle after unmount or navigation. |
| `UI_PARENT` | native | error | A node listed before its parent reached the native decoder. | Rebuild the page with a matching @lingxia/view runtime and host SDK. |
| `UI_PATCH` | native | error | An invalid JSON state patch arrived from Logic. | Mutate page data only through page.update or page.set with plain JSON values. |
| `UI_PERF` | build | warning | More than 50 literal rows sit outside <List>, an opacity/backdrop layer repeats per row, or a bundled image exceeds 2048 px or 8 MiB. | Use <List> for long lists, move backdrop/opacity to the container or use a translucent role, and downsize large images. |
| `UI_PLOT` | runtime, native | error | One or more <plot> props (kind, values, domain, colors, pieLayout, selectedIndex) are invalid. | Pass kind (bar\|line\|pie), 1..128 finite values, domain containing 0 and all values, and 1..16 chart roles. |
| `UI_POPOVER` | native | warning | An anchored popover height oscillated between capped and uncapped and was held capped. | Give the popover content a stable height or max-height. |
| `UI_PROP` | build, runtime, native | error | An element received a prop it does not accept, an invalid value, or a conflicting prop combination. | Remove the unsupported prop or fix its value/combination per elements.md and semantics.md. |
| `UI_RADIO` | runtime, native | error | A <radio> has invalid options, value, binding or label. | Pass 1..64 options with unique non-empty string values and labels, an aria-label, and a string bind or value + onChange. |
| `UI_RENDER` | runtime, native | error | State was mutated or an action/handler API was called during render, or render recursed. | Call .set(), .focus() and actions only inside event handlers, never during render. |
| `UI_REQUEST` | runtime | error | A useRequest call received invalid arguments or a non-unary action. | Pass a unique string name, a unary Logic action from view.actions, and optional JSON params. |
| `UI_ROOT` | runtime, native | error | A page render did not return exactly one root element. | Return a single <page> element from render(). |
| `UI_SCALE` | build, runtime, native | error | A theme scale step is combined with unsupported arithmetic or used on a field that does not accept steps. | Use a single pack step (or literal length) without combining two steps or applying min()/max() over steps. |
| `UI_SCOPE` | build, runtime | error | A hook or JSX element was evaluated outside render (including JSX at module scope). | Call hooks and construct JSX inside render() or a component/function invoked during render. |
| `UI_SCROLL` | native | error | The host reported an invalid scroll position. | Check the host scroll adapter offset reporting. |
| `UI_SELECT` | runtime, native | error | A <select> has invalid options, value, placeholder or selection reply. | Pass 1..64 unique { value, label } options, a string value/placeholder, and an aria-label. |
| `UI_SHADOW` | runtime, native | error | A boxShadow or ring definition is malformed or exceeds the 8-shadow limit. | Use Tailwind shadow-*/ring-* utilities or pass valid shadow/ring objects within bounds. |
| `UI_SLIDER` | runtime, native | error | A <slider> range, step, value, values pair or label is invalid. | Set finite min < max, positive step dividing the range (<= 1e6 steps), aria-label, and valid value or values. |
| `UI_STALE` | runtime | error | A JSX element handle from an earlier render was reused in a later render. | Create fresh JSX elements inside each render instead of caching them across renders. |
| `UI_STATE` | native | error | Invalid initial or updated page state arrived from Logic. | Ensure Logic page data is plain JSON without functions, symbols or circular references. |
| `UI_STREAM` | runtime | error | A useStream call, listener, sequence or streaming action failed validation. | Pass a unique name, an async* streaming action from view.actions, and a function for onData. |
| `UI_STYLE` | build, runtime, native | error | A style property is unknown, unsupported on the element, or uses an invalid combination (sticky outside scroll, layout in state paint). | Use only properties accepted by that element in styles.md and paint-only properties inside state variants. |
| `UI_STYLE_STATIC` | build | warning | An inline style object contains only fixed literal values. | Move fixed styles to Tailwind utilities in className and reserve style for computed values. |
| `UI_STYLE_VALUE` | build, runtime, native | error | A style value is out of range or of the wrong type/format. | Pass a value in the accepted type and range named in the message. |
| `UI_SURFACE` | native | recovery | The platform GPU surface was lost or resized mid-frame. | No authoring change required; the host recreates the surface and redraws. |
| `UI_TAG` | build, runtime, native | error | An element tag is not part of the View language. | Replace it with a supported View element from elements.md (e.g. <view>, <text>, <image>). |
| `UI_TEST_ID` | native | error | An invalid data-testid reached the native decoder. | Use a non-empty string within identifierBytes for data-testid. |
| `UI_TEXT` | build, runtime, native | error | A <span> sits outside <text>, a link label is invalid, the runs of a <text> hold more than `textBytes` of text, or a component inside <text> returns a non-run. | Keep <span> inside <text>, split long rich text across <text> elements, and return only runs, popovers or dialogs inside <text>. |
| `UI_TIMEOUT` | native | recovery | View code repeatedly exceeded its CPU time budget or hung. | Optimize or move expensive work to Logic, then reload the page. |
| `UI_TRANSFORM` | build, native | error | A scaled or rotated box holds an editor, scroll, slider or webview, or a native transform is singular. | Translate instead of scaling/rotating containers holding native controls, or transform a sibling. |
| `UI_TRANSITION` | native | error | An invalid transition definition reached the renderer. | Use transition-* utilities with supported painted properties and durations <= 10 s. |
| `UI_VECTOR` | build, runtime, native | error | A <vector> has invalid paths, viewBox, gradients, stroke or transform props. | Pass 1..32 valid path objects with SVG d strings, a 4-number viewBox, and <= 8 gradients. |
| `UI_VIDEO` | runtime, native | error | A <video> source, sources list, poster or control prop is invalid. | Pass a valid src or sources array (not both), valid poster URL, and boolean/number control props in range. |
| `UI_VIEWPORT` | native | error | The host reported an invalid viewport size. | Check the host surface viewport dimensions. |
| `UI_WEBVIEW` | runtime, native | error | A <webview> source, session handle, label or callback is invalid. | Pass a valid src or useWebView() handle, an aria-label, and function callbacks. |
| `UI_WORKER` | native | recovery | The View worker thread failed to start. | Restart the app or check host OS thread limits. |
