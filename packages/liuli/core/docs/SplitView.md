<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# SplitView

A sidebar beside its content for a wide window, each scrolling on its own. Status: preview.

```ts
import { SplitView } from '@liulidesign/core/SplitView';
```

SplitView lays out a workspace: a fixed-width sidebar, usually a NavList, and the content it chooses beside it. Both sit on the canvas, parted by a hairline, and each pane scrolls on its own. The content is capped, so lines stay readable, and starts a fixed gutter from the sidebar: it reads from the sidebar outward, and a wider window adds room on the far side.

## Examples

### A mail workspace

```tsx
import { useLocal } from '@lingxia/view';
import { NavList } from '@liulidesign/core/NavList';
import { SplitView } from '@liulidesign/core/SplitView';

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

/** A mail workspace: the sidebar chooses a place, the content shows it at once. */
export function Workspace() {
  const current = useLocal<string>('current', 'inbox');
  const title = titles[current.value] ?? '';
  return (
    <view className="h-[360px] overflow-hidden rounded-container border border-text/12">
      <SplitView
        sidebarLabel="Mail"
        contentLabel={title}
        sidebarWidth={200}
        contentKey={current.value}
        sidebar={<NavList label="Mailboxes" sections={sections} value={current.value} onChange={next => current.set(next)} data-testid="workspace-nav" />}
        data-testid="workspace"
      >
        <text role="heading" headingLevel={1} className="text-title font-semibold text-text" data-testid="workspace-title">{title}</text>
        <text className="text-body text-muted">The content pane scrolls on its own and keeps its gutter from the sidebar in a wide window.</text>
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
| `sidebarLabel` (required) | `string` |  | Names the sidebar for assistive tech: "Components". |
| `contentLabel` (required) | `string` |  | Names the content: the open page's title. |
| `sidebarWidth` | `number` | `264` | The sidebar's width in px. |
| `contentKey` | `string` |  | Names what the content shows; a new key starts the content pane at its top, as opening a page does. |
| `contentWidth` | `SplitViewContentWidth` | `'wide'` | How wide the content may grow; past it the surplus falls on the far side. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the root; the panes get `<testid>-sidebar` and `<testid>-content`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | Fills its page: the two panes side by side. |
| sidebar | `<view>` | yes | The sidebar pane, `sidebarWidth` wide, a group named by `sidebarLabel`, with a hairline at its end. |
| content | `<view>` | yes | The content pane, taking the rest of the width; a group named by `contentLabel`. |
| scroll | `<scroll>` | yes | In each pane, so the two scroll apart. |
| sidebarInner | `<view>` | yes | The sidebar's padding. |
| contentInner | `<view>` | yes | The content's padding and cap, held to the start of its pane. |

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

- Do: Use it on a wide window (from the `md` breakpoint) and show phones one pane at a time, as separate pages.
- Do: Update `contentLabel` with the open page's title.
- Do: Use `contentWidth="prose"` for reading and `wide` for examples or tables.
- Don't: Put it inside a `<scroll>`; it scrolls each pane itself and must fill its page.
- Don't: Put a second sidebar in the content; nest a NavList section instead.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A sidebar that collapses and slides over the content on a narrow window | Nothing in the View; planned in Liuli | A fixed sidebar on wide windows; phones use pages |
| Dragging the hairline to resize the sidebar | Pointer coordinates on an element (`onPointer`) | A fixed `sidebarWidth` |

Related: NavList.
