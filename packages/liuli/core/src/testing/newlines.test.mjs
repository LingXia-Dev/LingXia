import assert from 'node:assert/strict';
import { test } from 'node:test';
import { renderPage } from './harness.mjs';

// Body text given as a string keeps the author's line breaks; a label stays on one line.
test('a newline in a description or message breaks the line; a newline in a label does not', async () => {
  const screen = await renderPage(`
    import { Banner } from '@liulidesign/core/Banner';
    import { Button } from '@liulidesign/core/Button';
    import { Item, ItemList } from '@liulidesign/core/Item';
    export function render() {
      return <page>
        <ItemList label="Device"><Item label="Printer" description={'Firmware 2.1.0\\nUptime 3 days 4 hr'} /></ItemList>
        <Banner title="Update" message={'Restarting.\\nBack in a minute.'} />
        <Button label={'Save\\nnow'} onClick={() => {}} data-testid="save" />
      </page>;
    }
  `);
  const space = text => screen.queryAll(node => node.tag === 'text' && node.text === text)[0]?.props.whiteSpace;
  assert.equal(space('Firmware 2.1.0\nUptime 3 days 4 hr'), 'pre-line');
  assert.equal(space('Restarting.\nBack in a minute.'), 'pre-line');
  const label = screen.queryAll(node => node.tag === 'text' && node.text === 'Save\nnow')[0];
  assert.ok(label, 'the label is drawn');
  assert.equal(label.props.whiteSpace, undefined);
});
