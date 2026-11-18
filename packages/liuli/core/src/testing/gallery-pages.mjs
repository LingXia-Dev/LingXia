import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { render } from '@lingxia/view/testing';

// `npm run test:pages`, out of `npm test` for its minutes: every gallery page opens in both forms.
// The device refuses a scaled or rotated editor, scroll or slider when it paints, which Node never
// does, so the tree is checked for that here.
const gallery = resolve(dirname(fileURLToPath(import.meta.url)), '../../../gallery');
const pages = JSON.parse(readFileSync(resolve(gallery, 'lxapp.json'), 'utf8')).pages.map(page => page.name);
const forms = {
  phone: { env: { sizeClass: 'compact', viewport: { width: 393, height: 852 } } },
  desk: { env: { sizeClass: 'regular', viewport: { width: 1280, height: 800 } } },
};

const turned = transform => {
  if (!transform) return false;
  const { scale, rotate } = JSON.parse(transform);
  return (scale && (scale[0] !== 1 || scale[1] !== 1)) || (rotate !== undefined && rotate % 360 !== 0);
};
const held = new Set(['input', 'textarea', 'scroll', 'slider']);

for (const page of pages) {
  for (const [form, input] of Object.entries(forms)) {
    test(`${page} (${form}): opens, and nothing scaled or rotated holds an editor, a scroll or a slider`, async () => {
      const screen = await render(resolve(gallery, `pages/${page}/index.tsx`), input, { root: gallery });
      for (const node of screen.queryAll(each => turned(each.props.transform))) {
        const inside = [];
        const walk = each => { if (held.has(each.tag)) inside.push(each.tag); for (const child of each.children ?? []) walk(child); };
        walk(node);
        assert.deepEqual(inside, [], `${page}: a ${node.tag} at ${node.props.transform} holds ${inside.join(', ')}`);
      }
    });
  }
}
