<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Tour

A few callouts, one at a time, each pointing at something. Status: preview.

```ts
import { Tour } from '@liulidesign/core/Tour';
```

Tour shows one callout at a time beside an element of the page: a title, a sentence or two, where the tour is, and buttons to go on, go back or skip. Each step names its target by `id`; the callout sits under, over or beside it and flips for room. The app holds which step is in view, so it can start, end and remember the tour. The page is not dimmed and stays usable: the callout points, it does not block.

## Examples

### Three steps

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { IconButton } from '@liulidesign/core/IconButton';
import { SearchField } from '@liulidesign/core/SearchField';
import { Tour, type TourStep } from '@liulidesign/core/Tour';

const steps: TourStep[] = [
  { target: 'tour-new', targetLabel: 'The New note button', title: 'Start a note', text: 'A note opens ready to type, and saves as you go.' },
  { target: 'tour-search', targetLabel: 'The search field', title: 'Find anything', text: 'Search reads every note, list and photo.' },
  { target: 'tour-share', targetLabel: 'The Share button', title: 'Share what you choose', text: 'A note is private until you invite someone.', placement: 'top' },
];

/** Three things on a small screen, shown one at a time; the tour can be skipped or stepped back. */
export function Welcome() {
  const step = useLocal<number | null>('step', null);
  const query = useLocal('query', '');
  return (
    <view className="gap-4">
      <view className="flex-row items-center gap-2">
        <view id="tour-new"><Button label="New note" variant="primary" startIcon="add" /></view>
        <view id="tour-search" className="grow basis-0 min-w-0">
          <SearchField label="Search notes" value={query.value} onChange={query.set} placeholder="Search" clearLabel="Clear" />
        </view>
      </view>
      <view className="flex-row items-center justify-between gap-3">
        <text className="text-body text-text">Trip to the coast</text>
        <view id="tour-share"><IconButton label="Share" icon="share" /></view>
      </view>
      <Button label="Show me around" width="content" onClick={() => step.set(0)} data-testid="tour-start" />
      <text role="status" className="text-footnote text-muted">{step.value === null ? 'Tour not running' : `On step ${step.value + 1}`}</text>
      <Tour
        steps={steps}
        step={step.value}
        onStepChange={next => step.set(next)}
        nextLabel="Next"
        backLabel="Back"
        skipLabel="Skip"
        doneLabel="Done"
        stepLabel={(at, count) => `${at + 1} of ${count}`}
        data-testid="tour"
      />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `steps` (required) | `readonly TourStep[]` |  |  |
| `step` (required) | `number \| null` |  | The step in view, from 0, or null while the tour is not running. The app holds it. |
| `onStepChange` (required) | `(step: number \| null) => void` |  | Runs with the next step to show, and with null when the tour ends: finished, or skipped. |
| `onFinish` | `(() => void)` |  | Runs when the last step\'s button is pressed, before the tour ends. |
| `nextLabel` (required) | `string` |  | "Next". |
| `backLabel` (required) | `string` |  | "Back". |
| `skipLabel` (required) | `string` |  | "Skip": ends the tour from any step but the last. |
| `doneLabel` (required) | `string` |  | "Done": on the last step. |
| `stepLabel` (required) | `(step: number, count: number) => string` |  | Where the tour is, written by the app: `(step, count) => \`${step + 1} of ${count}\``. |
| `data-testid` | `string` |  | On the callout. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| caret | `<view>` | yes | A small point on the edge facing the target, anchored to the target, so it stays on the target's centre when the callout is shifted to fit. |
| callout | `<view>` | yes | A popover hung from the caret, on the inverted surface. |
| title | `<text>` | yes | The step's title, a heading. |
| text | `<text>` | yes | One or two sentences. |
| count | `<text>` | yes | Where the tour is, from `stepLabel`. |
| actions | `<view>` | yes | Skip, Back and Next; Done on the last step. |

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- Tab: Through the callout's buttons.
- Enter or Space: Presses the button.
- Press target: sm 32 px.
- The callout is a group named for what it points at and its title: "The New note button: Start a note".
- It has no backdrop, so the page under it keeps its taps and its focus.
- When the tour ends the callout closes and focus stays where the user left it; if it was on a callout button, move it to the control the tour started from.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| callout text | `surface` on `text` | 4.5:1 | 16.26:1 | 13.87:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Keep a tour to three or four steps, about things already in view.
- Do: Let people skip it, and offer it again from Help.
- Don't: Use a tour to explain a screen that needs explaining; fix the screen.
- Don't: Start it over something the user is in the middle of.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| undefined | undefined | undefined |
| undefined | undefined | undefined |
| undefined | undefined | undefined |
| undefined | undefined | undefined |
| undefined | undefined | undefined |
| undefined | undefined | undefined |

Related: Popover, Tooltip, Banner.
