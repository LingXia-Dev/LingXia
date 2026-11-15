import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'TreeList',
  category: 'navigation',
  status: 'preview',
  since: '2026-11-15',
  summary: 'Things inside things, opened in place',
  description:
    'TreeList shows nested nodes as rows set in by their depth: folders and files, the sections of a document. A branch carries a chevron and opens or closes in place; any row can be the selected one. ' +
    'The app keeps the selection, and may keep the open branches too. For one level of destinations in groups, use NavList.',
  keywords: ['tree', 'tree view', 'outline', 'folders', 'hierarchy', 'nested', 'file tree'],
  propsType: 'TreeListProps',
  recipe: { map: 'treeListRecipe', parts: 'treeListParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The tree, named by `label`.' },
    { part: 'row', required: true, description: 'One node: a tree item set in by its level.' },
    { part: 'chevron', required: false, description: 'On a branch; it turns when the branch opens. A leaf keeps its width empty.' },
    { part: 'icon', required: false, description: 'An icon before the label; not read.' },
    { part: 'label', required: true, description: 'The node\'s name, cut short with an ellipsis.' },
    { part: 'end', required: false, description: 'A count or a size at the end of the row.' },
  ],
  accessibility: {
    pattern: 'tree',
    name: 'label',
    description: [],
    states: ['expanded', 'selected', 'disabled'],
    keyboard: [
      { keys: 'Up and Down', effect: 'Move between the rows in view' },
      { keys: 'Enter or Space', effect: 'Selects the row; on a branch it also opens or closes it' },
    ],
    target: { md: 36 },
    contrast: [
      { what: 'label', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'selected row', pair: ['text', 'accent-soft'], minimum: '4.5:1' },
      { what: 'count', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'Each row is a tree item that reports its level, whether it is selected, and on a branch whether it is open.',
      'A row with `endText` is read as "label, end text".',
      'The selected row is told apart by its fill and its weight.',
    ],
  },
  guidance: {
    do: [
      'Keep it to a few levels; past six, rows stop stepping in.',
      'Open the branches that lead to the current row, so it is in view.',
    ],
    dont: [
      'Use a tree for a flat list in groups; that is NavList.',
      'Put actions in the rows; a row selects, and a branch also opens.',
    ],
  },
  examples: [
    { name: 'Folders', title: 'Folders and notes' },
    { name: 'Outline', title: 'An outline, opened from outside' },
  ],
  gaps: [
    'Left and Right do not close and open a branch: the View steps a tree with Up and Down only.',
    'Rows are not checked, dragged or loaded on demand.',
  ],
  related: ['NavList', 'Details', 'Breadcrumb'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Nested, in place',
    order: 8,
    page: {
      lead: 'Use a tree for things inside things: folders, an outline.',
      hero: 'Folders',
      variants: [{ name: 'Outline', title: 'Without icons, three levels deep' }],
    },
  },
};
