# RTL and i18n

```tsx
import { useView, type VectorPath } from '@lingxia/ui';

const messages = {
  en: { title: 'Settings', account: 'Account', language: 'Language' },
  ar: { title: 'الإعدادات', account: 'الحساب', language: 'اللغة' },
} as const;

const chevron: VectorPath[] = [{ d: 'M9 6l6 6-6 6', fill: 'none', stroke: 'currentColor', strokeWidth: 2 }];

export function render() {
  const { env } = useView();
  const t = env.displayLanguage.startsWith('ar') ? messages.ar : messages.en;
  return (
    <page className="p-4 gap-2">
      <text role="heading" className="text-xl font-semibold text-start">{t.title}</text>
      {[t.account, t.language].map(label => (
        <button key={label} onClick={() => {}} className="rounded-xl bg-surface text-text ps-4 pe-2 py-3">
          <row className="items-center gap-3">
            <text className="grow text-start text-sm">{label}</text>
            <vector aria-hidden paths={chevron} className="w-5 h-5 text-faint rtl:-scale-x-100" />
          </row>
        </button>
      ))}
      <row dir="ltr" className="gap-1">
        <text className="text-sm font-mono">+1 555 0100</text>
      </row>
    </page>
  );
}
```

## Direction

`env.direction` is `'rtl'` when the display language is written right to left (Arabic, Hebrew,
Persian, Urdu, …), otherwise `'ltr'`. Every element inherits it. `dir="rtl"` or `dir="ltr"` on an element
sets the direction of its subtree, for content whose direction does not follow the UI language (phone
numbers, code, a quoted message). `dir="auto"` takes the direction of the first strong character of the
element's content, as in HTML, for user text such as a chat message; an editor follows the text typed.

In a right-to-left subtree, rows run from right to left, grid columns are placed from the right, and
`text-start` aligns right. Horizontal scrolls start at their right edge.

## Logical properties

Use flow-relative utilities for anything that depends on reading direction:

| Logical | Physical equivalent in LTR |
| --- | --- |
| `ps-*`, `pe-*` | `pl-*`, `pr-*` |
| `ms-*`, `me-*` | `ml-*`, `mr-*` |
| `start-*`, `end-*` | `left-*`, `right-*` |
| `border-s-*`, `border-e-*` | `border-l-*`, `border-r-*` |
| `rounded-s-*`, `rounded-e-*`, `rounded-ss-*`, `rounded-se-*`, `rounded-es-*`, `rounded-ee-*` | the matching corners |
| `text-start`, `text-end` | `text-left`, `text-right` |

They stay logical until the host lays out the frame and resolve against each node's own direction, so a
language change mirrors the page without your code doing anything. Physical utilities (`pl-*`, `left-*`,
`text-left`) stay physical. When an element sets both for the same side, the logical one wins.
`space-x-*` and `divide-x` are logical already.

`*-block` utilities (`pbs-*`, `mbe-*`) map to top and bottom; `inset-bs-*`/`inset-be-*` are not supported.

## `rtl:` and `ltr:`

`rtl:` and `ltr:` match an element's inherited direction, including a `dir` override, the way `dark:`
matches the theme. Use them for what logical properties cannot express, chiefly mirroring icons that
point along the reading direction:

```tsx
import { type VectorPath } from '@lingxia/ui';

const back: VectorPath[] = [{ d: 'M15 6l-6 6 6 6', fill: 'none', stroke: 'currentColor', strokeWidth: 2 }];

export function render() {
  return (
    <page dir="rtl" className="p-4">
      <button onClick={() => {}} aria-label="Back" className="self-start p-3 text-text">
        <vector aria-hidden paths={back} className="w-6 h-6 rtl:-scale-x-100" />
      </button>
    </page>
  );
}
```

Do not mirror icons that show real-world things (a clock, a play button, a check mark).

## Display language

`env.displayLanguage` is the BCP 47 tag of the UI language (`en-US`, `zh-CN`, `ar`); the page
re-renders when it changes. It is also the shaping locale, so Han characters use the regional face
that language expects.

The View has no `Intl`. Keep message tables in plain objects as above, or let Logic translate and pass
strings in `view.data`. Format numbers, currencies, dates and plural forms in Logic. Localize the
navigation bar title in the page config:

```json
{ "navigationBar": { "title": { "default": "Settings", "ar": "الإعدادات" } } }
```
