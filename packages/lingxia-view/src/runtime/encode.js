import { tags, field } from '../generated/wire.js';
import { limits } from '../generated/contract.js';
import { utf8Bytes } from './data.js';
import { render } from './state.js';
import { readNode, isNode } from './nodes.js';
import { keyPayload } from './keys.js';

import { resolveNode, handlesIssued } from './elements.js';

const structured = new Set(['backgroundGradient', 'boxShadow', 'animation']);
const encodeValue = (key, value) => structured.has(key) && value !== 'none' ? JSON.stringify(value) : value;

export function copyTree(node) {
  // Encoding mutates list windows/maps only; ordinary resolved nodes can be shared.
  let children = node.children;
  for (let i = 0; i < children.length; i++) {
    const child = copyTree(node.children[i]);
    if (child === node.children[i]) continue;
    if (children === node.children) children = children.slice();
    children[i] = child;
  }
  if (node.list) return { ...node, children, list: { ...node.list, states: new Map(node.list.states), cache: new Map(node.list.cache), pure: new Set(node.list.pure) } };
  return children === node.children ? node : { ...node, children };
}

export function retainCells(cells) {
  for (const [key, cell] of cells) {
    render.candidate.locals.set(key, cell);
    for (const collector of render.collectors) collector.set(key, cell);
  }
}

const initialWindow = count => ({ start: 0, end: Math.min(1, count), leading: 0, trailing: Math.max(0, count - 1) * 64 });

// Encoded subtrees by resolved node. Resolved nodes do not change, so a node a later render
// reuses (a recalled component, a cached row) copies its encoding instead of writing it again.
const slices = new WeakMap();
/** Nodes worth keeping an encoding for: what a later render is likely to reuse whole. */
export const sliceable = new WeakSet();

// Buffers are numbered (a trailing header); the host decodes each and acknowledges the last one it
// holds. A reused subtree written in that buffer goes as a reference to its nodes there.
const WIRE_SERIAL = -3, WIRE_REUSE = -2;
let wireSerial = 0, wireHeld = 0;
/** The host decoded buffer `serial` and keeps its nodes for the next one to reference. */
export function wireAck(serial) { wireHeld = serial; }
/** The host keeps no earlier buffer (a reset page): write every subtree in full. */
export function wireReset() { wireHeld = 0; }

/** The flat wire buffer of a resolved tree; list rows render for the requested windows. */
export function encodeTree(tree, windows) {
  const { candidate } = render;
  const serial = ++wireSerial;
  const output = []; let count = 0, plotWork = 0, animationBytes = 0, itemKeyBytes = 0, dataBytes = 0;
  // Header offsets, registered callbacks, lists written and the deepest level, for cutting slices.
  const heads = [], handled = []; let lists = 0, deepest = 0;
  const paste = (slice, parent, depth) => {
    if (depth + slice.depth > limits.depth || (count += slice.nodes) > limits.nodes) throw Error('UI_LIMIT: candidate too large');
    if ((plotWork += slice.plotWork) > 16384) throw Error('UI_LIMIT: plot geometry budget exceeded');
    if ((animationBytes += slice.animationBytes) > limits.animationBytes) throw Error('UI_ANIMATION: page payload budget exceeded');
    if ((dataBytes += slice.dataBytes) > limits.dataAttributeBytes) throw Error('UI_PROP: data-* page budget exceeded');
    for (let i = 0; i < slice.handlers.length; i++) { const entry = slice.handlers[i]; candidate.callbacks.set(entry[0], entry[1]); handled.push(entry); }
    const base = output.length, values = slice.values;
    deepest = Math.max(deepest, depth + slice.depth);
    if (slice.serial === wireHeld && wireHeld) {
      output.push(WIRE_REUSE, parent, slice.serial, slice.offset, slice.nodes, 0);
      slice.serial = serial; slice.offset = base;
      // Its nodes are not in this buffer, so no slice cut around it can stand alone.
      heads.push(-1);
      return;
    }
    slice.serial = serial; slice.offset = base;
    for (let i = 0; i < values.length; i++) output.push(values[i]);
    output[base + 1] = parent;
    for (let i = 0; i < slice.heads.length; i++) {
      const at = slice.heads[i];
      if (at) output[base + at + 1] += base;
      heads.push(base + at);
    }
  };
  const write = (node, parent, depth) => {
    const slice = slices.get(node);
    if (slice) return paste(slice, parent, depth);
    if (!sliceable.has(node)) return writeNode(node, parent, depth);
    const index = output.length, mark = { heads: heads.length, handled: handled.length, lists, deepest, count, plotWork, animationBytes, dataBytes };
    deepest = depth;
    writeNode(node, parent, depth);
    const heads2 = heads.slice(mark.heads);
    if (lists === mark.lists && !heads2.includes(-1)) {
      const values = output.slice(index), at = heads2.map(head => head - index);
      for (let i = 1; i < at.length; i++) values[at[i] + 1] -= index;
      slices.set(node, { values, heads: at, handlers: handled.slice(mark.handled), depth: deepest - depth, nodes: count - mark.count,
        plotWork: plotWork - mark.plotWork, animationBytes: animationBytes - mark.animationBytes, dataBytes: dataBytes - mark.dataBytes,
        serial, offset: index });
    }
    deepest = Math.max(deepest, mark.deepest);
  };
  const writeNode = (node, parent, depth) => {
    if (depth > limits.depth || ++count > limits.nodes) throw Error('UI_LIMIT: candidate too large');
    deepest = Math.max(deepest, depth);
    const { tag, props, identity, fields, states, wire, handlers } = node;
    // Field pairs go straight after the node's header; its pair count is filled in last.
    const index = output.length;
    heads.push(index);
    output.push(tags[tag], parent, props.key ?? '', props.testId ?? '', node.text, 0);
    const start = output.length;
    for (const key in fields) {
      const value = fields[key];
      if (key === 'animation') {
        animationBytes += utf8Bytes(JSON.stringify(value), 32768);
        if (animationBytes > limits.animationBytes) throw Error('UI_ANIMATION: page payload budget exceeded');
      }
      output.push(field[key], encodeValue(key, value));
    }
    for (const key in states) {
      const style = states[key], pairs = [];
      for (const name in style) pairs.push(field[name], encodeValue(name, style[name]));
      if (pairs.length) output.push(field[key], JSON.stringify(pairs));
    }
    for (let i = 0; i < wire.length; i++) {
      const key = wire[i][0], value = wire[i][1];
      if (key === 'dataAttributes') {
        dataBytes += utf8Bytes(value, limits.dataAttributeBytes);
        if (dataBytes > limits.dataAttributeBytes) throw Error('UI_PROP: data-* page budget exceeded');
      }
      output.push(field[key], value);
    }
    for (let i = 0; i < handlers.length; i++) {
      const handler = handlers[i];
      candidate.callbacks.set(handler[1], handler[2]); handled.push([handler[1], handler[2]]); output.push(field[handler[0]], handler[1]);
    }
    if (node.plotWork) {
      plotWork += node.plotWork;
      if (plotWork > 16384) throw Error('UI_LIMIT: plot geometry budget exceeded');
    }
    if (node.rowIndex !== undefined) output.push(field.listRowIndex, node.rowIndex);
    if (node.slot !== undefined) output.push(field.childSlot, node.slot);
    let window, seededAnchor = false;
    if (node.list) {
      lists++;
      window = windows[identity] ?? node.list.window ?? initialWindow(props.count);
      if (!Number.isSafeInteger(window.start) || !Number.isSafeInteger(window.end) || window.start < 0 ||
          window.end < window.start || window.end > Math.max(props.count, node.list.previousCount) ||
          ![window.leading, window.trailing].every(v => Number.isFinite(v) && v >= 0 && v <= 1638400000)) throw Error('UI_LIST: invalid window');
      // A full render may shrink the list before native can reconcile its old window.
      if (window.end > props.count) window = initialWindow(props.count);
      const typography = node.list.rowSource ? JSON.stringify(node.typography ?? null) : undefined;
      if (node.list.reusable && node.list.reusable.typography === typography) {
        node.list.cache = new Map(node.list.reusable.rows);
        node.list.pure = new Set(node.list.reusable.rows.keys());
      }
      node.list.reusable = null;
      node.list.typography = typography;
      const target = props.anchor === 'end' ? props.count - 1 : Math.min(props.anchor?.index ?? 0, props.count - 1);
      // A gliding anchor scrolls natively: the window follows the scroll, not the target.
      if (node.list.anchorChanged && !props.anchor?.duration && props.count > 0 && (target < window.start || target >= window.end)) {
        // Keep old geometry only until native chooses the destination window.
        // These rows cannot be committed for an anchor outside this window.
        node.list.cache = new Map([...node.list.anchorCache].filter(([i, row]) =>
          props.itemKeys === undefined || row.props.key === `key:${JSON.stringify(props.itemKeys[i])}`));
        seededAnchor = true;
      }
      node.list.anchorChanged = false;
      node.list.anchorCache = null;
      node.list.previousCount = props.count;
      node.list.window = { ...window };
      output.push(field.listId, identity, field.listCount, props.count, field.listLeading, window.leading, field.listTrailing, window.trailing);
      if (props.itemKeys !== undefined) {
        const { payload, bytes } = keyPayload(props.itemKeys);
        itemKeyBytes += bytes;
        if (itemKeyBytes > limits.itemKeyBytes) throw Error('UI_LIST: itemKeys page budget exceeded');
        output.push(field.listItemKeys, payload);
      }
      if (props.anchor !== undefined) output.push(field.listAnchor, JSON.stringify(props.anchor));
      candidate.lists.set(identity, node.list);
    }
    output[start - 1] = (output.length - start) / 2;
    if (!node.list) { const children = node.children; for (let i = 0; i < children.length; i++) write(children[i], index, depth + 1); return; }
    writeRows(node, window, index, depth, write);
    // Even overlapping destination rows must resolve against this render's props.
    if (seededAnchor) node.list.cache.clear();
  };
  write(tree, -1, 0);
  output.push(WIRE_SERIAL, serial, '', '', '', 0);
  return output;
}

function writeRows(node, window, index, depth, write) {
  const visibleKeys = new Set();
  for (const i of node.list.cache.keys()) if (i < window.start || i >= window.end) { node.list.cache.delete(i); node.list.pure.delete(i); }
  for (let i = window.start; i < window.end; i++) {
    const cells = new Map(); render.collectors.push(cells);
    try {
      let row = node.list.cache.get(i), key = node.list.states.get(i)?.key;
      if (!row) {
        const { candidate } = render, handles = handlesIssued(), declarations = candidate.declarations.size, resources = candidate.resources.size;
        const reads = render.cellReads;
        render.readsView = false;
        ({ row, key } = renderRow(node, i, cells, depth));
        // Reusable in a later render only if it holds no state, callback or resource and read no state or View.
        const pure = cells.size === 0 && !render.readsView && handlesIssued() === handles && render.cellReads === reads &&
          candidate.declarations.size === declarations && candidate.resources.size === resources;
        if (pure) node.list.pure.add(i); else node.list.pure.delete(i);
      } else { row = copyTree(row); retainCells(node.list.states.get(i).locals); }
      if (visibleKeys.has(key)) throw Error('UI_KEY: duplicate list row');
      visibleKeys.add(key);
      sliceable.add(row);
      write(row, index, depth + 1);
      node.list.cache.set(i, row);
      node.list.states.set(i, { key, locals: cells });
    } finally { render.collectors.pop(); }
  }
  // Rows outside the window keep an entry only for state to come back to.
  for (const [i, state] of node.list.states) {
    if (state.locals.size) retainCells(state.locals);
    else if (i < window.start || i >= window.end) node.list.states.delete(i);
  }
}

export function renderRow(node, i, cells, depth) {
  const { props, identity } = node;
  const suppliedKey = props.itemKeys?.[i];
  const previous = render.scope;
  render.scope = suppliedKey === undefined ? `${identity}/row:${i}` : `${identity}/row/key:${JSON.stringify(suppliedKey)}`;
  try {
    const value = props.row(i);
    const declared = isNode(value) ? readNode(value)[1].key : undefined;
    if (declared !== undefined && typeof declared !== 'string') throw Error('UI_KEY: expected string');
    if (suppliedKey === undefined && declared !== undefined && cells.size > 0)
      throw Error('UI_LIST: keyed row factory hooks require itemKeys; alternatively move hooks into a keyed component');
    if (suppliedKey !== undefined && declared !== undefined && suppliedKey !== declared) throw Error('UI_LIST: row key does not match itemKeys');
    const rowKey = suppliedKey ?? declared;
    const key = rowKey === undefined ? `index:${i}` : `key:${JSON.stringify(rowKey)}`;
    render.scope = `${identity}/row/${key}`;
    const fields = {};
    if (props.height) {
      const height = props.height(i);
      if (!Number.isFinite(height) || height < 0 || height > 16384) throw Error('UI_LIST: invalid row height');
      fields.height = height;
    }
    // Rows are siblings in the list, so structural variants see their row index.
    const content = resolveNode(value, render.scope, depth + 2, { typography: node.typography, sibling: { index: i, count: props.count } });
    if (content?.fragment) throw Error('UI_LIST: a row returns one node; wrap its fragment in <column>');
    const row = { tag: 'column', element: 'column', props: { key }, fields, states: {}, wire: [], handlers: [], identity: render.scope,
      rowIndex: i, text: '', children: content === null ? [] : [content] };
    return { row, key };
  } finally { render.scope = previous; }
}
