_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Variants

Every Tailwind variant, tested against the native lowering. Supported variants may be stacked
(`md:active:bg-surface-sunken`). Conditions on DOM state the View does not have are refused at build time.

## Supported

| Variant | Applies to |
| --- | --- |
| `placeholder:` | editors; `text-*` color only |
| `first:` | any element |
| `last:` | any element |
| `only:` | any element |
| `odd:` | any element |
| `even:` | any element |
| `empty:` | any element |
| `hover:` | any element, only when `env.pointer` is `fine`; paint only |
| `focus:` | `<input>` `<textarea>` `<search>`; paint only |
| `active:` | `<button>` `<checkbox>` `<switch>` `<radio>` `<link>`; paint only |
| `disabled:` | `<button>` `<input>` `<textarea>` `<search>` `<checkbox>` `<switch>` `<radio>` `<slider>` `<link>` `<select>`; replaces the whole class cascade |
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

`space-x-*`, `space-y-*` and `divide-*` are utilities, not variants: they style every child but the last.

## Refused

| Variant | Probed as | Reason |
| --- | --- | --- |
| `*:` | `*:` | child variants (put the classes on each child, e.g. when mapping the children in TS) |
| `**:` | `**:` | descendant variants (put the classes on the descendants that need them) |
| `not:` | `not-hover:` | selector :not(:hover) |
| `group:` | `group-hover:` | group-*/peer-* variants (hold the shared state in TS and choose both elements' classes from it) |
| `peer:` | `peer-hover:` | group-*/peer-* variants (hold the shared state in TS and choose both elements' classes from it) |
| `first-letter:` | `first-letter:` | selector ::first-letter |
| `first-line:` | `first-line:` | selector ::first-line |
| `marker:` | `marker:` | selector  ::marker |
| `selection:` | `selection:` | selector  ::selection |
| `file:` | `file:` | selector ::file-selector-button |
| `backdrop:` | `backdrop:` | selector ::backdrop |
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
| `indeterminate:` | `indeterminate:` | selector :indeterminate |
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
| `focus-within:` | `focus-within:` | selector :focus-within |
| `focus-visible:` | `focus-visible:` | selector :focus-visible |
| `enabled:` | `enabled:` | selector :enabled |
| `inert:` | `inert:` | selector :is([inert], [inert] *) |
| `in:` | `in-focus:` | ancestor-state variants (in-*; track the ancestor state in TS, e.g. onFocus/onBlur, and choose the classes from it) |
| `has:` | `has-checked:` | selector :has(:checked) (has-[…]: compute the condition in TS and choose the class, e.g. hasError ? "border-error" : "border-input") |
| `aria:` | `aria-checked:` | selector [aria-checked="true"] (aria-*: choose classes from the state in TS and set accessibility props such as role separately) |
| `data:` | `data-[open]:` | selector [data-open] (data-[…]: there are no data attributes; choose classes from the state in TS) |
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
| `starting:` | `starting:` | nested selector |
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
