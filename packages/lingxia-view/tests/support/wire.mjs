// Decodes wire buffers into nodes with named fields so tests assert behavior, not ids.
import { tags, field } from '../../src/generated/wire.js';
import { describe } from '../../src/internal.js';

const tagNames = Object.fromEntries(Object.entries(tags).map(([name, id]) => [id, name]));
const fieldNames = Object.fromEntries(Object.entries(field).map(([name, id]) => [id, name]));
const states = new Set(['focusStyle', 'pressedStyle', 'hoverStyle']);

export function decode(buffer) {
  const nodes = [], offsets = new Map();
  for (let cursor = 0; cursor < buffer.length;) {
    const [tag, parent, key, testId, text, count] = buffer.slice(cursor, cursor + 6);
    // The buffer's serial number heads it; tests never acknowledge, so no reference follows.
    if (tag === -3) { cursor += 6; continue; }
    const props = {};
    for (let i = 0; i < count; i++) {
      const name = fieldNames[buffer[cursor + 6 + i * 2]], value = buffer[cursor + 7 + i * 2];
      props[name] = states.has(name) ? Object.fromEntries(pairs(JSON.parse(value))) : value;
    }
    const node = { tag: tagNames[tag], key, testId, text, props, children: [] };
    offsets.set(cursor, node);
    if (parent >= 0) offsets.get(parent).children.push(node);
    nodes.push(node);
    cursor += 6 + count * 2;
  }
  return nodes;
}

function pairs(flat) {
  const result = [];
  for (let i = 0; i < flat.length; i += 2) result.push([fieldNames[flat[i]], flat[i + 1]]);
  return result;
}

/** The root of a decoded buffer. */
export const tree = buffer => decode(buffer)[0];

/** Describe-and-commit, decoded to the root node. */
export const rendered = (render, view = {}) => tree(describe(render, view));

export function find(root, test) {
  if (test(root)) return root;
  for (const child of root.children) { const found = find(child, test); if (found) return found; }
}

export const byTestId = (root, id) => find(root, node => node.testId === id);
export const byText = (root, text) => find(root, node => node.text === text);
