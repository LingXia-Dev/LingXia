// In-process helpers over the public decoder, for tests that drive the runtime directly.
import { describe } from '../../src/internal.js';
import { decode, find } from '../../src/testing.js';

export { decode, find };

/** The root of a decoded buffer. */
export const tree = buffer => decode(buffer)[0];

/** Describe-and-commit, decoded to the root node. */
export const rendered = (render, view = {}) => tree(describe(render, view));

export const byTestId = (root, id) => find(root, node => node.testId === id);
export const byText = (root, text) => find(root, node => node.text === text);

/** Theme steps resolved under the built-in pack, as `@lingxia/view/testing` exports it. */
export { points } from '../../src/testing.js';
