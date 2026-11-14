import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'SplitView',
  category: 'content',
  status: 'preview',
  since: '2026-11-14',
  summary: 'A sidebar beside its content for a wide window, each scrolling on its own',
  description:
    'SplitView lays out a workspace: a fixed-width sidebar, usually a NavList, and the content it chooses beside it. ' +
    'Both sit on the canvas, parted by a hairline, and each pane scrolls on its own. ' +
    'The content is capped, so lines stay readable, and starts a fixed gutter from the sidebar: it reads from the sidebar outward, and a wider window adds room on the far side.',
  keywords: ['split view', 'sidebar layout', 'master detail', 'two pane', 'workspace', 'navigation split', 'app shell'],
  propsType: 'SplitViewProps',
  recipe: { map: 'splitViewRecipe', parts: 'splitViewParts' },
  anatomy: [
    { part: 'root', required: true, description: 'Fills its page: the two panes side by side.' },
    { part: 'sidebar', required: true, description: 'The sidebar pane, `sidebarWidth` wide, a group named by `sidebarLabel`, with a hairline at its end.' },
    { part: 'content', required: true, description: 'The content pane, taking the rest of the width; a group named by `contentLabel`.' },
    { part: 'scroll', required: true, description: 'In each pane, so the two scroll apart.' },
    { part: 'sidebarInner', required: true, description: 'The sidebar\'s padding.' },
    { part: 'contentInner', required: true, description: 'The content\'s padding and cap, held to the start of its pane.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [],
    contrast: [],
    notes: [
      'Each pane is a group named by its label, so a screen reader can move between them; the content\'s name should follow the open page.',
      'Tab moves from the sidebar into the content in reading order.',
      'The hairline is decoration; the panes are told apart by their names, not by the line.',
    ],
  },
  guidance: {
    do: [
      'Use it on a wide window (from the `md` breakpoint) and show phones one pane at a time, as separate pages.',
      'Update `contentLabel` with the open page\'s title.',
      'Use `contentWidth="prose"` for reading and `wide` for examples or tables.',
    ],
    dont: [
      'Put it inside a `<scroll>`; it scrolls each pane itself and must fill its page.',
      'Put a second sidebar in the content; nest a NavList section instead.',
    ],
  },
  examples: [
    { name: 'Workspace', title: 'A mail workspace' },
  ],
  gaps: [
    { missing: 'A sidebar that collapses and slides over the content on a narrow window', needs: 'Nothing in the View; planned in Liuli', today: 'A fixed sidebar on wide windows; phones use pages' },
    { missing: 'Dragging the hairline to resize the sidebar', needs: 'Pointer coordinates on an element (`onPointer`)', today: 'A fixed `sidebarWidth`' },
  ],
  related: ['NavList'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Sidebar and content',
    order: 5,
    page: {
      lead: 'Use a split view for a workspace with a sidebar on a wide window.',
      stage: 'bare',
      hero: 'Workspace',
    },
  },
};
