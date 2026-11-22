<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# SplitView

A sidebar beside its content, each scrolling on its own; one pane at a time on a phone. Status: preview.

```ts
import { SplitView } from '@liulidesign/core/SplitView';
```

SplitView lays out a workspace: a fixed-width sidebar, usually a NavList, and the content it chooses beside it. Both sit on the canvas, parted by a hairline, and each pane scrolls on its own. The content is capped, so lines stay readable, and starts a fixed gutter from the sidebar: it reads from the sidebar outward, and a wider window adds room on the far side. In the compact size class there is no room for two panes, so it shows one: the sidebar, then the content `pane` asks for, under a back button that leads to the sidebar again.

## Examples

### A mail workspace

```tsx
import { useLocal } from '@lingxia/view';
import { NavList } from '@liulidesign/core/NavList';
import { SplitView, type SplitViewPane } from '@liulidesign/core/SplitView';

const sections = [
  { id: 'mail', items: [
    { value: 'inbox', label: 'Inbox', endText: '12' },
    { value: 'drafts', label: 'Drafts', endText: '3' },
    { value: 'sent', label: 'Sent' },
  ] },
  { id: 'projects', title: 'Projects', items: [
    { value: 'launch', label: 'Spring launch' },
    { value: 'research', label: 'User research' },
  ] },
] as const;
const titles: Record<string, string> = { inbox: 'Inbox', drafts: 'Drafts', sent: 'Sent', launch: 'Spring launch', research: 'User research' };

/** A mail workspace: the sidebar chooses a place, the content shows it at once; on a phone the place opens over the list, with a way back. */
export function Workspace() {
  const current = useLocal<string>('current', 'inbox');
  const pane = useLocal<SplitViewPane>('pane', 'sidebar');
  const title = titles[current.value] ?? '';
  return (
    <view className="h-[360px] overflow-hidden rounded-container border border-text/12">
      <SplitView
        sidebarLabel="Mail"
        contentLabel={title}
        pane={pane.value}
        onBack={() => pane.set('sidebar')}
        sidebarWidth={200}
        contentKey={current.value}
        sidebar={<NavList label="Mailboxes" sections={sections} value={current.value} onChange={next => { current.set(next); pane.set('content'); }} data-testid="workspace-nav" />}
        data-testid="workspace"
      >
        <text role="heading" headingLevel={1} className="text-title font-semibold text-text" data-testid="workspace-title">{title}</text>
        <text className="text-body text-muted">Each pane scrolls on its own. Beside the sidebar the content keeps a gutter from it; on a phone it takes the screen.</text>
      </SplitView>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `sidebar` (required) | `Child` |  | The sidebar's content, usually a NavList; it scrolls on its own. |
| `children` (required) | `Child` |  | The content beside it; it scrolls on its own. |
| `contentActions` | `Child` |  | Actions on what the content shows, such as Approve: a row under the content, outside its scroll, so they stay in view. |
| `sidebarLabel` | `string` |  | Names the sidebar for assistive tech: "Components". Defaults to "Sidebar" in the page's language. |
| `contentLabel` (required) | `string` |  | Names the content: the open page's title. |
| `pane` | `SplitViewPane` | `'sidebar'` | On a compact screen one pane shows at a time: which. A regular screen shows both and ignores it. |
| `onBack` | `(() => void)` |  | On a compact screen: the back button over the content was pressed; show the sidebar. |
| `backLabel` | `string` | ``sidebarLabel`` | The back button's label: where it leads. |
| `sidebarWidth` | `number` | `264` | The sidebar's width in px, beside the content. |
| `contentKey` | `string` |  | Names what the content shows; a new key starts the content pane at its top, as opening a page does. |
| `contentWidth` | `SplitViewContentWidth` | `'wide'` | How wide the content may grow; past it the surplus falls on the far side. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the root; the panes get `<testid>-sidebar` and `<testid>-content`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | Fills its page: the two panes side by side, or the one pane a compact screen shows. |
| pane | `<view>` | no | Compact: the one pane on show, a group named like the pane it is; the content slides in from the end edge and out again, unless motion is reduced. |
| back | `<view>` | no | Compact, over the content: the row of the back button, outside the content's scroll. |
| sidebar | `<view>` | yes | The sidebar pane, `sidebarWidth` wide, a group named by `sidebarLabel`, with a hairline at its end. |
| content | `<view>` | yes | The content pane, taking the rest of the width; a group named by `contentLabel`. |
| scroll | `<scroll>` | yes | In each pane, so the two scroll apart. |
| sidebarInner | `<view>` | yes | The sidebar's padding. |
| contentInner | `<view>` | yes | The content's padding and cap, held to the start of its pane. |
| actions | `<view>` | no | With `contentActions`: a row under the content's scroll, behind a hairline, that stays in view. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `splitViewRecipe` from `@liulidesign/core/SplitView/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- It reads no Liuli scale token.
- Colour roles it paints with, set by the theme pack: `canvas`, `text`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- Each pane is a group named by its label, so a screen reader can move between them; the content's name should follow the open page.
- Tab moves from the sidebar into the content in reading order.
- The hairline is decoration; the panes are told apart by their names, not by the line.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Set `pane` to `content` when a row of the sidebar is chosen and back to `sidebar` from `onBack`; a regular screen ignores both.
- Do: Name the back button for where it leads with `backLabel` when the sidebar's label does not say it.
- Do: Update `contentLabel` with the open page's title.
- Do: Use `contentWidth="prose"` for reading and `wide` for examples or tables.
- Do: Put the actions on the open record in `contentActions`, so a long record never hides them.
- Don't: Put it inside a `<scroll>`; it scrolls each pane itself and must fill its page.
- Don't: Put a second sidebar in the content; nest a NavList section instead.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| The system back gesture leading from the content to the sidebar on a compact screen | A back request from the host | The back button leads back |
| Dragging the hairline to resize the sidebar | Pointer coordinates on an element (`onPointer`) | A fixed `sidebarWidth` |

Related: NavList.
