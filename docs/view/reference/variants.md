_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Variants

Every Tailwind variant, tested against the native lowering. Supported variants may be stacked
(`md:active:bg-surface-sunken`). Conditions on DOM state the View does not have are refused at build time.

## Supported

| Variant | Applies to |
| --- | --- |
| `placeholder:` | editors; `text-*` color only |
| `backdrop:` | `<dialog>` only; styles its scrim, which fades in and out on the dialog's own timing; `backdrop:starting:`/`backdrop:exit:` (paint only) and `backdrop:duration-*`/`ease-*`/`delay-*` change that |
| `first:` | any element |
| `last:` | any element |
| `only:` | any element |
| `odd:` | any element |
| `even:` | any element |
| `indeterminate:` | any element |
| `empty:` | any element |
| `focus-within:` | any element; while it or a descendant has focus; paint only |
| `hover:` | any element, while a mouse, stylus or trackpad is over it; paint only |
| `focus:` | `<input>` `<textarea>` `<select>`; paint only |
| `focus-visible:` | `<button>` `<input>` `<textarea>` `<checkbox>` `<switch>` `<radio>` `<slider>` `<link>` `<select>`; while focused from a key, switch or assistive tool (editors on any focus); paint only |
| `active:` | `<button>` `<checkbox>` `<switch>` `<radio>` `<slider>` `<link>`; paint only |
| `disabled:` | `<button>` `<input>` `<textarea>` `<checkbox>` `<switch>` `<radio>` `<slider>` `<link>` `<select>`; replaces the whole class cascade |
| `has:` | any element, alone; `has-checked:`, `has-disabled:` (a descendant checkbox, switch, answered radio or disabled element) and `has-focus:`; paint only |
| `data:` | any element; matches its own `data-*` prop (`data-active:`, `data-[state=open]:`), layout included |
| `motion-safe:` | any element |
| `motion-reduce:` | any element |
| `max:` | any element |
| `sm:` | any element |
| `md:` | any element |
| `lg:` | any element |
| `xl:` | any element |
| `2xl:` | any element |
| `min:` | any element |
| `ltr:` | any element |
| `rtl:` | any element |
| `dark:` | any element; not on colours, which follow appearance |
| `starting:` | any element; paint only, the values it enters from |
| `exit:` | any element; paint only, the values a removed element leaves toward |
| `track:` | `<slider>` and `<switch>`; its track: `bg-*`, `h-*`, `rounded-*`, and `w-*` on a switch; after `active:` `hover:` `focus-visible:` or `disabled:`, and `checked:` on a switch, eased over 150 ms |
| `range:` | `<slider>`; its range, min to value: `bg-*`, `h-*`, `rounded-*`; after `active:` `hover:` `focus-visible:` or `disabled:`, eased over 150 ms |
| `secondary:` | `<slider>`; its secondary range, min to `secondaryValue`: `bg-*`, `h-*`, `rounded-*`; after `active:` `hover:` `focus-visible:` or `disabled:`, eased over 150 ms |
| `thumb:` | `<slider>` and `<switch>`; its thumb: `bg-*`, `size-*`, `rounded-*`, `border-*`, `shadow-*`, `ring-*` (with `ring-offset-*`, `inset-ring-*`), `scale-*`; after a state like the track |
| `group-active:` | any element inside a pressable control; while that control is pressed; paint only |

`space-x-*`, `space-y-*` and `divide-*` are utilities, not variants: they style every child but the last.

## Refused

| Variant | Probed as | Reason |
| --- | --- | --- |
| `*:` | `*:` | *: places grid children only (col-start-*, col-end-*, col-span-*, row-start-*, row-end-*, row-span-*); put other classes on each child |
| `**:` | `**:` | descendant variants (put the classes on the descendants that need them) |
| `not:` | `not-hover:` | selector :not(:hover) |
| `group:` | `group-hover:` | group-*/peer-* variants (hold the shared state in TS and choose both elements' classes from it) |
| `peer:` | `peer-hover:` | group-*/peer-* variants (hold the shared state in TS and choose both elements' classes from it) |
| `first-letter:` | `first-letter:` | selector ::first-letter |
| `first-line:` | `first-line:` | selector ::first-line |
| `marker:` | `marker:` | selector  ::marker |
| `selection:` | `selection:` | selector  ::selection |
| `file:` | `file:` | selector ::file-selector-button |
| `details-content:` | `details-content:` | selector ::details-content |
| `before:` | `before:` | selector ::before |
| `after:` | `after:` | selector ::after |
| `first-of-type:` | `first-of-type:` | selector :first-of-type |
| `last-of-type:` | `last-of-type:` | selector :last-of-type |
| `only-of-type:` | `only-of-type:` | selector :only-of-type |
| `visited:` | `visited:` | selector :visited |
| `target:` | `target:` | selector :target |
| `open:` | `open:` | selector :is([open], :popover-open, :open) |
| `default:` | `default:` | selector :default |
| `checked:` | `checked:` | selector :checked |
| `placeholder-shown:` | `placeholder-shown:` | selector :placeholder-shown |
| `autofill:` | `autofill:` | selector :autofill |
| `optional:` | `optional:` | selector :optional |
| `required:` | `required:` | selector :required |
| `valid:` | `valid:` | selector :valid |
| `invalid:` | `invalid:` | selector :invalid |
| `user-valid:` | `user-valid:` | selector :user-valid |
| `user-invalid:` | `user-invalid:` | selector :user-invalid |
| `in-range:` | `in-range:` | selector :in-range |
| `out-of-range:` | `out-of-range:` | selector :out-of-range |
| `read-only:` | `read-only:` | selector :read-only |
| `enabled:` | `enabled:` | selector :enabled |
| `inert:` | `inert:` | selector :is([inert], [inert] *) |
| `in:` | `in-focus:` | ancestor-state variants (in-*; use focus-within: on the ancestor, or track the state in TS and choose the classes from it) |
| `aria:` | `aria-checked:` | selector [aria-checked="true"] (aria-*: choose classes from the state in TS and set aria-* props separately) |
| `nth:` | `nth-3:` | selector :nth-child(3) |
| `nth-last:` | `nth-last-3:` | selector :nth-last-child(3) |
| `nth-of-type:` | `nth-of-type-3:` | selector :nth-of-type(3) |
| `nth-last-of-type:` | `nth-last-of-type-3:` | selector :nth-last-of-type(3) |
| `supports:` | `supports-[display:grid]:` | nested selector |
| `contrast-more:` | `contrast-more:` | media query (prefers-contrast: more) |
| `contrast-less:` | `contrast-less:` | media query (prefers-contrast: less) |
| `@max:` | `@max-md:` | nested selector |
| `@:` | `@md:` | nested selector |
| `@min:` | `@min-md:` | nested selector |
| `portrait:` | `portrait:` | media query (orientation: portrait) |
| `landscape:` | `landscape:` | media query (orientation: landscape) |
| `print:` | `print:` | media query print |
| `forced-colors:` | `forced-colors:` | media query (forced-colors: active) |
| `inverted-colors:` | `inverted-colors:` | media query (inverted-colors: inverted) |
| `pointer-none:` | `pointer-none:` | media query (pointer: none) |
| `pointer-coarse:` | `pointer-coarse:` | media query (pointer: coarse) |
| `pointer-fine:` | `pointer-fine:` | media query (pointer: fine) |
| `any-pointer-none:` | `any-pointer-none:` | media query (any-pointer: none) |
| `any-pointer-coarse:` | `any-pointer-coarse:` | media query (any-pointer: coarse) |
| `any-pointer-fine:` | `any-pointer-fine:` | media query (any-pointer: fine) |
| `noscript:` | `noscript:` | media query (scripting: none) |
