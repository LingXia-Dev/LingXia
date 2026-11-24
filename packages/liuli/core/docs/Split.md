<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Split

The main content and a companion beside it: side by side on a wide screen, one above the other on a half-open fold, stacked or in a sheet on a phone. Status: preview.

```ts
import { Split } from '@liulidesign/core/Split';
```

Split pairs the main content (a video, a document, a basket) with a companion `aside` (a live chat, comments, an inspector, a summary). On a wide screen the aside sits at the end, `asideWidth` wide, and at a fold that crosses the page it takes the end leaf. On a foldable stood half open like a laptop, the main content takes the upper half and the aside the lower. On a phone, `compact="stack"` puts the aside under the main content, and `compact="sheet"` puts it in a bottom sheet the page opens.

## Examples

### A live stream and its chat

```tsx
import { cx, useLocal, useView } from '@lingxia/view';
import { Avatar } from '@liulidesign/core/Avatar';
import { Badge } from '@liulidesign/core/Badge';
import { Composer } from '@liulidesign/core/Composer';
import { Icon } from '@liulidesign/core/Icon';
import { IconButton } from '@liulidesign/core/IconButton';
import { MessageBubble } from '@liulidesign/core/MessageBubble';
import { Split } from '@liulidesign/core/Split';

const chat = [
  { name: 'Mira Patel', text: 'The light on the water right now!' },
  { name: 'Sam Ortiz', text: 'Which harbour is this?' },
  { name: 'Ana Lima', text: 'The north pier, the boats leave at seven.' },
  { name: 'Kai Chen', text: 'Watching from the train, the sound is perfect.' },
];

/** A live stream and its chat: side by side on a wide screen, the chat under the stream on a phone. */
export function LiveStreamSplit() {
  const { env } = useView();
  const draft = useLocal('chat-draft', '');
  const playing = useLocal('playing', true);
  const muted = useLocal('muted', false);
  const large = useLocal('large', false);
  return (
    <view className={cx('overflow-hidden rounded-container border border-text/12', env.sizeClass === 'regular' ? 'h-[400px]' : 'h-[600px]')}>
      <Split
        asideWidth={300}
        asideLabel="Live chat"
        aside={
          <view className="grow min-h-0">
            <view className="flex-row items-center justify-between px-4 pt-3 pb-2">
              <text role="heading" headingLevel={2} className="text-callout font-semibold text-text">Live chat</text>
              <text className="text-caption text-muted tabular-nums">1,284 watching</text>
            </view>
            <scroll className="grow min-h-0">
              <view className="px-4 pb-2 gap-2">
                {chat.map(line => (
                  <MessageBubble key={line.name} author={line.name} avatar={<Avatar name={line.name} size="sm" isDecorative />} text={line.text} />
                ))}
              </view>
            </scroll>
            <view className="px-3 pb-3 pt-2 border-t border-text/8">
              <Composer bind={draft} label="Message" placeholder="Say something" onSubmit={() => draft.set('')} />
            </view>
          </view>
        }
        data-testid="live-split"
      >
        <view className="grow min-h-0 p-4 gap-3">
          <view className="grow min-h-0 rounded-control bg-media-canvas items-center justify-center">
            <Icon name="video" size="lg" className="text-on-media/60" />
            <view className="absolute start-3 top-3">
              <Badge label="Live" tone="danger" emphasis="solid" />
            </view>
          </view>
          <view className="flex-row items-center gap-1">
            <IconButton label={playing.value ? 'Pause' : 'Play'} icon={playing.value ? 'pause' : 'play'} variant="ghost" onClick={() => playing.set(!playing.value)} />
            <IconButton label={muted.value ? 'Unmute' : 'Mute'} icon={muted.value ? 'mute' : 'volume'} variant="ghost" onClick={() => muted.set(!muted.value)} />
            <view className="grow shrink min-w-0 ps-2">
              <text className="text-footnote font-semibold text-text truncate">Evening boats at the north pier</text>
              <text className="text-caption text-muted truncate">Harbour Lights · started 42 minutes ago</text>
            </view>
            <IconButton label={large.value ? 'Exit full screen' : 'Full screen'} icon={large.value ? 'collapse' : 'expand'} variant="ghost" onClick={() => large.set(!large.value)} />
          </view>
        </view>
      </Split>
    </view>
  );
}
```

### A document and its comments

```tsx
import { useLocal, useView } from '@lingxia/view';
import { Avatar } from '@liulidesign/core/Avatar';
import { Button } from '@liulidesign/core/Button';
import { Composer } from '@liulidesign/core/Composer';
import { Split } from '@liulidesign/core/Split';

const comments = [
  { name: 'Ana Lima', time: '09:12', text: 'Can the legend sit under the map on a phone? It covers the north loop.' },
  { name: 'Kai Chen', time: '10:40', text: 'Agree. And the closed paths need more than colour to tell them apart.' },
  { name: 'Mira Patel', time: 'Yesterday', text: 'The elevation numbers read well at this size.' },
];

/** A document and its comments: beside it on a wide screen, in a sheet the reader opens on a phone. */
export function DocumentSheetSplit() {
  const { env } = useView();
  const commentsOpen = useLocal('comments-open', false);
  const reply = useLocal('reply', '');
  const isCompact = env.sizeClass !== 'regular';
  return (
    <view className="h-[440px] overflow-hidden rounded-container border border-text/12">
      <Split
        compact="sheet"
        bind={commentsOpen}
        asideLabel="Comments"
        asideWidth={300}
        aside={
          <view className="h-full min-h-0">
            <text role="heading" headingLevel={2} className="px-4 pt-3 pb-2 text-callout font-semibold text-text">Comments</text>
            <scroll className="grow min-h-0">
              <view className="px-4 pb-3 gap-4">
                {comments.map(comment => (
                  <view key={comment.name} className="flex-row gap-3">
                    <Avatar name={comment.name} size="sm" isDecorative />
                    <view className="grow shrink min-w-0 gap-0.5">
                      <view className="flex-row items-baseline justify-between gap-2">
                        <text className="text-footnote font-semibold text-text">{comment.name}</text>
                        <text className="text-caption text-muted">{comment.time}</text>
                      </view>
                      <text className="text-footnote text-text">{comment.text}</text>
                    </view>
                  </view>
                ))}
              </view>
            </scroll>
            <view className="px-4 pt-3 border-t border-text/8" data-testid="doc-reply-row">
              <Composer bind={reply} label="Reply" placeholder="Add a comment" onSubmit={() => reply.set('')} data-testid="doc-reply" />
            </view>
          </view>
        }
        data-testid="doc-split"
      >
        <scroll className="grow min-h-0">
          <view className="p-4 gap-3">
            <view className="gap-1">
              <text role="heading" headingLevel={2} className="text-title font-semibold tracking-tight text-text">Trail map redesign</text>
              <text className="text-caption text-muted">Draft · edited 2 hours ago</text>
            </view>
            {isCompact && (
              <Button
                label="Comments (3)"
                variant="secondary"
                width="content"
                startIcon="message"
                onClick={() => commentsOpen.set(true)}
                data-testid="doc-open-comments"
              />
            )}
            <text role="heading" headingLevel={3} className="text-callout font-semibold text-text">Goals</text>
            <text className="text-body text-text">Walkers should find the loop that suits the time they have, and see where a path is closed before they reach it.</text>
            <text role="heading" headingLevel={3} className="text-callout font-semibold text-text">What changes</text>
            <text className="text-body text-text">The map keeps the coast at the top. Each loop gets one colour and a pattern, its length in kilometres and its climb in metres beside the name.</text>
            <text className="text-body text-text">Closed paths are drawn dashed with a short reason, such as a fallen tree or a nesting season.</text>
          </view>
        </scroll>
      </Split>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `compact` | `"sheet" \| "stack"` | `'stack'` | `stack` puts the aside under the main content; `sheet` puts it in a bottom sheet the page opens. |
| `bind` | `Local<boolean>` |  | A View-local cell: whether the sheet is open. |
| `isOpen` | `boolean` |  | Whether the sheet is open. |
| `onOpenChange` | `((isOpen: boolean) => void)` |  | Runs with `false` when the sheet is dismissed. |
| `children` | `Child` |  | The main content: a video, a document, a canvas. |
| `aside` (required) | `Child` |  | The companion beside it: a live chat, comments, an inspector. |
| `asideLabel` (required) | `string` |  | Names the companion for a screen reader in every form, beside, below or in a sheet: "Live chat", "Comments". Not drawn; start `aside` with its own heading. |
| `asideWidth` | `number` | `320` | The aside's width in pt beside the main content, when no fold crosses the page. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the root; the panes get `<testid>-primary` and `<testid>-aside`, the sheet `<testid>-sheet`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | Fills its page: a row on a wide screen, a column on a phone or a half-open fold. |
| primary | `<view>` | yes | The main content; what is inside it sizes itself to the pane's own width, not the screen's. |
| aside | `<view>` | yes | The companion, a group named by `asideLabel`; on a phone with `compact="sheet"`, the body of the sheet. |
| hingeGap | `<view>` | no | An empty strip as wide as the fold's crease, so nothing is drawn across it. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `splitRecipe` from `@liulidesign/core/Split/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- It reads no Liuli scale token.
- Colour roles it paints with, set by the theme pack: `canvas`, `text`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- The aside is a group named by `asideLabel`, so a screen reader can move to it; on a phone its sheet takes the same name, and Escape or Back closes it.
- The strip over a fold's crease is hidden from assistive tech.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use `compact="stack"` when both must stay in view on a phone, such as a live video above its chat.
- Do: Use `compact="sheet"`, with `bind` or `isOpen` and `onOpenChange`, and a button that shows only on a phone, when the companion can wait (comments, filters).
- Do: Start the companion with its own heading; `asideLabel` names it for a screen reader and the opening button, and is not drawn.
- Do: For an aside that ends in a field, such as a chat or comments with a reply: the list grows and scrolls, and the field is pinned last (`h-full` on the aside, a `grow min-h-0` scroll, then the field).
- Do: Let it fill the page to its bottom edge and across its width: the seam is placed from those edges, so it meets the crease whatever sits above.
- Don't: Read the fold or the platform in page code to move the video and the chat; Split does that.

Related: ListDetail, BottomSheet, Drawer.
