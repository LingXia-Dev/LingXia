# Component API Reference

This file is the API doc for all currently implemented components in `@lingxia/components`.

## Component List

| Component | Custom Element | React | Vue |
| --- | --- | --- | --- |
| Video | `lx-video` | `LxVideo` | `LxVideo` |
| Picker | `lx-picker` | `LxPicker` | `LxPicker` |
| Navigator | `lx-navigator` | `LxNavigator` | `LxNavigator` |
| Input | `lx-input` | `LxInput` | `LxInput` |
| Textarea | `lx-textarea` | `LxTextarea` | `LxTextarea` |

## Cross-cutting API Rules

- `data-*` attributes are included in native logic callback payload as `target.dataset` and `currentTarget.dataset`.
- `bindX` / `catchX` values are Page function names (`string`), not function references.
- `onX` is view-side callback only (React/Vue/custom element listener).
- Event names are matched case-insensitively for logic bindings.
- Public video loading event name is `waiting`.

## LxVideo API

### Import

```ts
import { LxVideo } from '@lingxia/components/react';
import { LxVideo } from '@lingxia/components/vue';
```

### Props

| Prop | Type | Default | Notes |
| --- | --- | --- | --- |
| `id` | `string` | auto-generated | Stable component identity |
| `src` | `string` | - | Media URL; changing it reloads the player source |
| `poster` | `string` | - | Placeholder image shown before first frame |
| `objectFit` | `"cover" \| "contain" \| "fill" \| "fit"` | - | Resize mode; unset falls back to native default |
| `rotate` | `0 \| 90 \| 180 \| 270` | - | Invalid values are ignored and clear rotation override |
| `autoplay` | `boolean` | `false` | Requests immediate play after source becomes ready |
| `loop` | `boolean` | `false` | Restarts automatically when playback reaches end |
| `muted` | `boolean` | `false` | Mutes audio output; commonly used with autoplay |
| `controls` | `boolean` | `true` | Shows built-in native control UI |
| `progressBar` | `boolean` | `true` | In live mode, progress interaction may be limited by runtime policy |
| `live` | `boolean` | `false` | Enables live-stream semantics; seek/progress behavior differs from VOD |
| `volume` | `number \| string` | - | Expected range is `0..1`; out-of-range values are clamped/ignored by platform |
| `qualities` | `Array<{ label: string; url?: string }>` | - | First item is default |
| `playbackRates` | `number[]` | - | First item is default |

### View Events

- React props: `onPlayRequest`, `onPlay`, `onPlaying`, `onPause`, `onStop`, `onEnded`, `onTimeUpdate`, `onError`, `onLoadedMetadata`, `onFullscreenChange`, `onWaiting`, `onQualityChange`, `onRateChange`
- Vue emits: `playRequest`, `play`, `playing`, `pause`, `stop`, `ended`, `timeUpdate`, `error`, `loadedMetadata`, `fullscreenChange`, `waiting`, `qualityChange`, `rateChange`
- Custom element events: `playrequest`, `play`, `playing`, `pause`, `stop`, `ended`, `timeupdate`, `error`, `loadedmetadata`, `fullscreenchange`, `waiting`, `qualitychange`, `ratechange`

### Logic Bindings

- Supported: `bindPlayRequest`, `bindPlay`, `bindPlaying`, `bindPause`, `bindStop`, `bindEnded`, `bindTimeUpdate`, `bindError`, `bindLoadedMetadata`, `bindFullscreenChange`, `bindWaiting`, `bindQualityChange`, `bindRateChange`
- `catch*` supports the same event set.
- Vue/custom-element attribute style `bind-play-request` and flat style `bindplayrequest` are both accepted.

## LxPicker API

### Import

```ts
import { LxPicker } from '@lingxia/components/react';
import { LxPicker } from '@lingxia/components/vue';
```

### Props

| Prop | Type | Default | Notes |
| --- | --- | --- | --- |
| `id` | `string` | auto-generated | Stable component identity |
| `mode` | `"selector" \| "multiSelector" \| "cascading" \| "date" \| "time"` | inferred | React/Vue wrapper infers selector mode from `columns` when not date/time |
| `columns` | `string[][] \| [string[], Record<string, string[]>]` | - | Selector/multi/cascading mode |
| `defaultIndex` | `number \| number[]` | `0` | Custom element input |
| `value` | `string \| string[]` | - | React/custom element |
| `modelValue` | `string \| string[]` | - | Vue |
| `start` | `string` | - | Date/time range start |
| `end` | `string` | - | Date/time range end |
| `fields` | `"year" \| "month" \| "day" \| "range"` | - | Date mode precision |
| `cancelText` | `string` | `""` | Empty means platform default text |
| `confirmText` | `string` | `""` | Empty means platform default text |
| `cancelButtonColor` | `string` | - | Native cancel button background color |
| `confirmButtonColor` | `string` | - | Native confirm button background color |
| `cancelTextColor` | `string` | - | Native cancel button text color |
| `confirmTextColor` | `string` | - | Native confirm button text color |

### Event Payload

```ts
type LxPickerEventDetail = {
  index?: number | number[];
  value?: string | string[];
  confirmed?: boolean;
  cancelled?: boolean;
};
```

### View Events

| Surface | API |
| --- | --- |
| React | `onConfirm(value)`, `onCancel()`, `onScroll(value)`, `onChange(event)`, `onNativeScroll(event)` |
| Vue | `confirm(value)`, `cancel()`, `scroll(value)`, `update:modelValue(value)` |
| Custom element | `change`, `scroll` |

### Logic Bindings

- Supported: `bindChange`, `bindScroll`, `catchChange`, `catchScroll`

## LxNavigator API

### Import

```ts
import { LxNavigator } from '@lingxia/components/react';
import { LxNavigator } from '@lingxia/components/vue';
```

### Props

| Prop | Type | Default | Notes |
| --- | --- | --- | --- |
| `url` | `string` | `""` | Target URL/path |
| `openType` | `"navigate" \| "redirect" \| "navigateBack" \| "reLaunch" \| "switchTab" \| "exit" \| "openUrl" \| "tel"` | `"navigate"` | Selects navigation behavior and target action; `tel` calls host `makePhoneCall` |
| `target` | `"self" \| "lxapp" \| "browser"` | auto-infer | Inferred by component when omitted |
| `delta` | `number` | `1` | Number of pages to pop for `navigateBack` |
| `appId` | `string` | - | Target lxapp |
| `path` | `string` | - | Target lxapp path; query string is supported |
| `phoneNumber` | `string` | - | Required when `openType="tel"` |
| `hoverClass` | `string` | `"navigator-hover"` | Hover/touch feedback class name |
| `hoverStopPropagation` | `boolean` | `false` | Stops hover touch event propagation when `true` |
| `hoverStartTime` | `number` | `20` | Delay before hover class is applied (ms) |
| `hoverStayTime` | `number` | `70` | Delay before hover class is removed (ms) |

### View Events

| Surface | API |
| --- | --- |
| React | `onSuccess`, `onFail`, `onComplete` |
| Vue | `success`, `fail`, `complete` |
| Custom element | `success`, `fail`, `complete` |

Event detail type: `{ success?: boolean; errMsg?: string }`

`tel` failure contract:

- If platform does not support phone dialing (for example macOS), the component dispatches `fail` and `complete` with `detail.success = false` and a non-empty `detail.errMsg`.
- The error is a view event contract; Navigator does not provide logic bindings for `fail`.

### Logic Bindings

- Navigator currently has no `bindX` / `catchX` API.

## LxInput API

### API Subset

LingXia keeps a focused cross-end compatible subset.

Aligned subset:

| Field | LingXia prop / attr | Notes |
| --- | --- | --- |
| `value` | `value` | Same semantics |
| `type` | `type` | Supports `text` / `number` / `password` / `digit` |
| `password` | `password` | Alias for password mode |
| `placeholder` | `placeholder` | Same semantics |
| `placeholder-style` | `placeholderStyle` / `placeholder-style` | Placeholder color is applied on Android/iOS/macOS; Harmony currently falls back to default placeholder color |
| `placeholder-class` | `placeholderClass` / `placeholder-class` | Same semantics |
| `disabled` | `disabled` | Same semantics |
| `maxlength` | `maxlength` | Same semantics |
| `cursor-spacing` | `cursorSpacing` / `cursor-spacing` | Same semantics |
| `auto-focus` | `autoFocus` / `auto-focus` | Alias of `focus` |
| `focus` | `focus` | Same semantics |
| `confirm-type` | `confirmType` / `confirm-type` | Same semantics |
| `always-embed` | `alwaysEmbed` / `always-embed` | Forwarded to native runtime |
| `confirm-hold` | `confirmHold` / `confirm-hold` | Same semantics |
| `cursor` | `cursor` | Same semantics |
| `cursor-color` | `cursorColor` / `cursor-color` | Forwarded to native runtime |
| `selection-start` / `selection-end` | `selectionStart` / `selectionEnd` | Same semantics |
| `adjust-position` | `adjustPosition` / `adjust-position` | Same semantics |
| `hold-keyboard` | `holdKeyboard` / `hold-keyboard` | Forwarded to native runtime |

Not currently exposed in LingXia `LxInput`:

- `type` values `idcard` / `safe-password` / `nickname`
- `safe-password-*` fields
- `selectionchange` / `keyboardcomposition*` / `worklet:onkeyboardheightchange` style hooks

### View Events

- React props: `onInput`, `onChange`, `onFocus`, `onBlur`, `onConfirm`, `onKeyboardHeightChange`, `onNicknameReview`
- Vue emits: `input`, `change`, `focus`, `blur`, `confirm`, `keyboardHeightChange`, `nicknameReview`
- Custom element events: `input`, `change`, `focus`, `blur`, `confirm`, `keyboardheightchange`, `nicknamereview`
- Platform note: `keyboardheightchange` is currently emitted on iOS and Harmony. `nicknamereview` is reserved and may not fire on all runtimes yet.

#### macOS platform notes

- **No soft keyboard**: macOS has no on-screen keyboard, so `keyboardheightchange` is never emitted and `adjust-position` has no effect.
- **Auto-blur**: tapping/clicking outside the native input hands focus back to the WebView through the normal macOS first-responder mechanism. Programmatic blur (`blur()` API or `focus: false` prop) works as expected. If your page relies on a tap-outside event to dismiss the input (common on mobile), you should also wire up a `mousedown` listener on the WebView side that explicitly calls `blur()`.

### Logic Bindings

- Supported: `bindInput`, `bindChange`, `bindFocus`, `bindBlur`, `bindConfirm`, `bindKeyboardHeightChange`, `bindNicknameReview`
- `catch*` supports the same event set.

## LxTextarea API

### API Subset

LingXia keeps a focused cross-end compatible subset.

Aligned subset:

| Field | LingXia prop / attr | Notes |
| --- | --- | --- |
| `value` | `value` | Same semantics |
| `placeholder` | `placeholder` | Same semantics |
| `placeholder-style` | `placeholderStyle` / `placeholder-style` | Placeholder color is applied on Android/iOS/macOS; Harmony currently falls back to default placeholder color |
| `placeholder-class` | `placeholderClass` / `placeholder-class` | Same semantics |
| `disabled` | `disabled` | Same semantics |
| `maxlength` | `maxlength` | Same semantics |
| `auto-focus` | `autoFocus` / `auto-focus` | Alias of `focus` |
| `focus` | `focus` | Same semantics |
| `auto-height` | `autoHeight` / `auto-height` | Same semantics |
| `cursor-spacing` | `cursorSpacing` / `cursor-spacing` | Same semantics |
| `show-confirm-bar` | `showConfirmBar` / `show-confirm-bar` | Same semantics |
| `adjust-position` | `adjustPosition` / `adjust-position` | Same semantics |
| `hold-keyboard` | `holdKeyboard` / `hold-keyboard` | Forwarded to native runtime |
| `disable-default-padding` | `disableDefaultPadding` / `disable-default-padding` | Forwarded to native runtime |
| `confirm-type` | `confirmType` / `confirm-type` | Supports `send/search/next/go/done/return`; default is `return` |
| `confirm-hold` | `confirmHold` / `confirm-hold` | Same semantics |
| `fixed` | `fixed` | Forwarded to native runtime |
| `adjust-keyboard-to` | `adjustKeyboardTo` / `adjust-keyboard-to` | Supports `cursor` / `bottom` |
| `cursor` | `cursor` | Same semantics |
| `selection-start` / `selection-end` | `selectionStart` / `selectionEnd` | Same semantics |

Not currently exposed in LingXia `LxTextarea`:

- `selectionchange` / `keyboardcomposition*` style hooks

### View Events

- React props: `onInput`, `onChange`, `onFocus`, `onBlur`, `onConfirm`, `onLineChange`, `onKeyboardHeightChange`
- Vue emits: `input`, `change`, `focus`, `blur`, `confirm`, `lineChange`, `keyboardHeightChange`
- Custom element events: `input`, `change`, `focus`, `blur`, `confirm`, `linechange`, `keyboardheightchange`
- Platform note: `keyboardheightchange` is currently emitted on iOS and Harmony.

#### macOS platform notes

- **No soft keyboard**: `keyboardheightchange` is never emitted, `adjust-position` has no effect.
- **Auto-blur**: same as LxInput — clicks outside hand focus to the WebView via the macOS first-responder system. Wire a `mousedown` listener on the WebView side to call `blur()` explicitly if needed.

### Logic Bindings

- Supported: `bindInput`, `bindChange`, `bindFocus`, `bindBlur`, `bindConfirm`, `bindLineChange`, `bindKeyboardHeightChange`
- `catch*` supports the same event set.
