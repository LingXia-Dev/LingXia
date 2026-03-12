# Component API Reference

This file is the API doc for all currently implemented components in `@lingxia/components`.

## Component List

| Component | Custom Element | React | Vue |
| --- | --- | --- | --- |
| Video | `lx-video` | `LxVideo` | `LxVideo` |
| Picker | `lx-picker` | `LxPicker` | `LxPicker` |
| Navigator | `lx-navigator` | `LxNavigator` | `LxNavigator` |

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
