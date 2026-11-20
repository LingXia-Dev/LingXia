import { expect, test } from '@lingxia/test';
import { tap } from './finger.js';

// An open dialog or popover panel takes every press over it; only a press outside dismisses it.

interface Panel { name: string; page: string; open: string; panel: string; heading: string; outside: 'above' | 'below' }
const panels: Panel[] = [
  { name: 'dialog', page: 'motion', open: 'open-dialog', panel: 'motion-dialog', heading: 'Turn on backups?', outside: 'above' },
  { name: 'popover', page: 'popover', open: 'options', panel: 'options-panel', heading: 'Show in the list', outside: 'below' },
];

for (const p of panels) {
  test(`${p.name}: a tap on its panel text or padding keeps it open; outside closes it`, {
    id: `PANEL-PRESS-${p.name.toUpperCase()}-001`, start: { page: p.page }, timeout: 90_000,
  }, async (t) => {
    const view = t.app.view;
    const panel = view.testId(p.panel);
    const open = async () => {
      if (!(await panel.isVisible())) await view.testId(p.open).click();
      await expect(panel).toBeVisible();
      await view.settled();
      return panel.boundingBox();
    };
    const box = await open();
    const head = await view.getByRole('heading', { name: p.heading }).boundingBox();
    const inside: Array<[string, [number, number]]> = [
      ['its heading text', [head.x + head.width / 2, head.y + head.height / 2]],
      ['its left padding', [box.x + 6, box.y + box.height / 2]],
      ['its top padding', [box.x + box.width / 2, box.y + 5]],
      ['its right padding', [box.x + box.width - 6, box.y + box.height / 2]],
      ['its bottom padding', [box.x + box.width / 2, box.y + box.height - 5]],
    ];
    for (const [what, at] of inside) {
      await open();
      await tap(view, at);
      expect(await panel.isVisible(), `a tap on ${what} keeps the ${p.name} open`).toBe(true);
    }
    const shown = await open();
    const vp = await view.viewport();
    const y = p.outside === 'above'
      ? Math.max(shown.y - 60, vp.top + 20)
      : Math.min(shown.y + shown.height + 80, vp.top + vp.height - 20);
    await tap(view, [vp.left + vp.width / 2, y]);
    await expect(panel).not.toBeVisible();
  });
}
