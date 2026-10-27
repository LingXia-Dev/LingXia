_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Error codes

Every message starts with its code: `CODE: detail`. **build** codes stop `lingxia build` with
`file:line:col`; **runtime** codes come from the View runtime with the JSX trace (`at <tag> (file:line:col)`);
**native** codes come from the host decoder, layout or renderer and keep the last good frame.

| Code | Raised at | Meaning |
| --- | --- | --- |
| `UI_ACTION` | runtime, native | An action call was malformed or its reply invalid. |
| `UI_ANIMATION` | build, runtime, native | An invalid or over-budget animation definition, or an inline `style.animation`. |
| `UI_BOOTSTRAP_TIMEOUT` | native | Logic did not deliver the first page state in time. |
| `UI_BRIDGE` | native | The View-Logic connection failed or was replaced. |
| `UI_BUDGET` | native | A warning, once per page and limit: a frame used at least three quarters of a page limit. |
| `UI_BUFFER` | native | A malformed wire buffer reached the native decoder. |
| `UI_BUTTON` | build, runtime, native | Invalid button label, a scroll or list inside a composed control, or a control whose only content is another control. |
| `UI_CHANNEL` | runtime | Invalid `useChannel` call or channel failure. |
| `UI_CHECK` | runtime, native | Invalid checkbox or switch value, label or binding. |
| `UI_CHILD` | runtime, native | An element has children it does not accept, or lacks its required parent. |
| `UI_CHROME` | native | Invalid host chrome geometry from the platform adapter. |
| `UI_CLASS` | runtime | A class string is not a string, too long, or names a utility the build did not compile. |
| `UI_CLASS_ELEMENT` | build | A supported utility on an element that does not render it, or a state variant the element never enters. |
| `UI_CLASS_UNKNOWN` | build | Not a Tailwind utility; the message names the nearest supported one when close. |
| `UI_CLASS_UNSUPPORTED` | build | Tailwind CSS the native renderer cannot draw. |
| `UI_COLOR` | build, runtime, native | A colour is not a role (`surface`, `accent/12`), `currentColor` or `transparent`, or a colour class uses `dark:`; hex is legal only in vectors, complete gradients and shadow layers. |
| `UI_COMMIT` | runtime | A commit without a prepared candidate. |
| `UI_COMPAT` | runtime, native | The installed host is older than the compiled page; update the host app. |
| `UI_COMPOSITE` | native | A composited effect (backdrop, opacity group) is out of bounds. |
| `UI_CSS` | build | The page stylesheet has something other than tokens, or more than one is imported. |
| `UI_DOM` | build | A DOM global (`window`, `document`, …) is used in View code. |
| `UI_ENVIRONMENT` | native | The host reported an invalid environment. |
| `UI_ERROR` | runtime | A failure without its own code. |
| `UI_FIELD` | native | A wire field on a tag that does not accept it. |
| `UI_FIELDS` | native | A malformed field list reached the native decoder. |
| `UI_FILTER` | native | A backdrop filter outside its bounds. |
| `UI_FONT` | native | A named system font in the platform fallback chain could not be read or found; text falls back to the next font. |
| `UI_FRAGMENT` | build | A JSX fragment; return one element. |
| `UI_FRAME` | native | The platform presenter kept refusing frames prepared ahead; the page falls back to one frame at a time. |
| `UI_GRADIENT` | runtime, native | A gradient with geometry its kind does not take, or invalid stops. |
| `UI_GRID` | native | An invalid grid track list or line. |
| `UI_IMAGE` | runtime, native | Invalid image source, alt, retry key or callback, or a failed load. |
| `UI_INPUT` | runtime, native | Invalid editor prop, binding or text. |
| `UI_KEY` | build, runtime, native | A non-string or duplicate sibling key, or a component keyed by its own function name. |
| `UI_LEAVE` | runtime, native | Invalid `useLeaveGuard` call or answer. |
| `UI_LIMIT` | runtime, native | A page limit was exceeded. |
| `UI_LIST` | runtime, native | Invalid list description, keys, anchor or row height. |
| `UI_LOCAL` | runtime | A duplicate `useLocal` name in one component, or `set` on an unmounted cell. |
| `UI_MODULE` | native | The compiled page module failed to load. |
| `UI_PAGE` | native | A call reached a page that was already discarded. |
| `UI_PARENT` | native | A node listed before its parent reached the native decoder. |
| `UI_PATCH` | native | An invalid state patch from Logic. |
| `UI_PERF` | build | A build warning; the page still builds: literal rows above 50 outside `<List>`, an opacity or backdrop layer repeated per row, or a bundled image above 2048 px or 8 MiB. |
| `UI_PLOT` | runtime, native | Invalid plot props. |
| `UI_PROP` | build, runtime, native | A prop the element does not take, or an invalid value. |
| `UI_RADIO` | runtime, native | Invalid radio options, value or label. |
| `UI_RENDER` | runtime, native | A state change during render, or a recursive render. |
| `UI_REQUEST` | runtime | Invalid `useRequest` call. |
| `UI_ROOT` | runtime, native | The page did not return exactly one element. |
| `UI_SCOPE` | runtime | A hook or JSX used outside render. |
| `UI_SCROLL` | native | Invalid scroll position from the host. |
| `UI_SELECT` | runtime, native | Invalid `<select>` options, value or selection reply. |
| `UI_SESSION` | native | A call reached a page session that is not loaded. |
| `UI_SHADOW` | runtime, native | An invalid or over-budget box shadow. |
| `UI_SLIDER` | runtime, native | Invalid slider range, step, value or label. |
| `UI_STALE` | runtime | A candidate from another render. |
| `UI_STATE` | native | Invalid page state from Logic. |
| `UI_STREAM` | runtime | Invalid `useStream` call or stream failure. |
| `UI_STYLE` | build, runtime, native | A style key the element does not render, or an invalid combination (sticky, state paint). |
| `UI_STYLE_VALUE` | runtime | A style value out of range or of the wrong kind. |
| `UI_SURFACE` | native | The platform surface was lost or resized mid-frame; the host recovers. |
| `UI_TAG` | build, runtime, native | Not an element of the View language. |
| `UI_TEST_ID` | native | An invalid `testId` reached the native decoder. |
| `UI_TEXT` | runtime, native | A `<span>`/`<link>` outside `<text>`, a bad link label, or too many runs. |
| `UI_TIMEOUT` | native | View code exceeded its time budget; the page needs a reload. |
| `UI_TRANSFORM` | native | A transform the element cannot take (rotate/scale on editors, scrolls, sliders). |
| `UI_TRANSITION` | native | An invalid transition definition. |
| `UI_VECTOR` | runtime, native | Invalid vector paths, gradients or viewBox. |
| `UI_VIEWPORT` | native | The host reported an invalid viewport. |
| `UI_WORKER` | native | The View worker thread failed to start. |
