import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderPage } from '../testing/harness.mjs';

const vectors = node => node.tag === 'vector' ? [node] : (node.children ?? []).flatMap(vectors);
const side = node => [node.props.width, node.props.height];

test('An icon beside text grows with it, up to half again; an icon-only button keeps its glyph', async () => {
  for (const [textScale, growth] of [[1.2, 1.2], [2, 1.5], [3, 1.5]]) {
    const screen = await renderPage(`
      import { Button } from '@liulidesign/core/Button';
      import { IconButton } from '@liulidesign/core/IconButton';
      import { Badge } from '@liulidesign/core/Badge';
      import { Banner } from '@liulidesign/core/Banner';
      import { Item } from '@liulidesign/core/Item';
      export function render() {
        return <page>
          <Button label="Run again" startIcon="refresh" onClick={() => {}} data-testid="button" />
          <IconButton icon="refresh" label="Run again" onClick={() => {}} data-testid="icon-only" />
          <Badge label="Synced" startIcon="check" data-testid="badge" />
          <Banner tone="info" title="Update ready" data-testid="banner" />
          <Item label="Wi-Fi" startIcon="wifi" data-testid="item" />
        </page>;
      }
    `, { env: { textScale } });
    const at = (id, base) => assert.deepEqual(side(vectors(screen.getByTestId(id))[0]), [Math.round(base * growth), Math.round(base * growth)], `${id} at ${textScale}`);
    at('button', 18); at('badge', 14); at('banner', 20);
    assert.deepEqual(side(vectors(screen.getByTestId('icon-only'))[0]), [18, 18], 'icon-only stays');
    const tile = screen.getByTestId('item').children[0].children[1];
    const tileGrowth = Math.min(textScale, 1.3);
    assert.deepEqual([side(tile), side(vectors(tile)[0])], [[Math.round(32 * tileGrowth), Math.round(32 * tileGrowth)], [Math.round(18 * tileGrowth), Math.round(18 * tileGrowth)]], `tile at ${textScale}`);
  }
});
