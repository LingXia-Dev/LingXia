import { expect, test, type Fixture } from '@lingxia/test';
import { rawAutomation } from '@lingxia/test/runner';
import { openUnderTabs, rootOf } from './open-demo';
import { decodePng } from './png.js';
import { apart, fillOf, sameHue } from './pixels';

type Shot = Awaited<ReturnType<typeof decodePng>>;

/** The pixels in a tab that differ from its own background: its icon and label, as drawn. */
function inkOf(shot: Shot, scale: number, origin: { left: number; top: number }, [x, y, w, h]: number[]) {
  const samples: number[][] = [];
  for (let row = 1; row < 16; row++) {
    for (let col = 1; col < 16; col++) {
      const px = Math.round((x - origin.left + (w * col) / 16) * scale);
      const py = Math.round((y - origin.top + (h * row) / 16) * scale);
      samples.push(shot.at(px, py));
    }
  }
  // The background is what most samples show; a tab's glyphs and text cover the rest.
  const key = (rgb: number[]) => rgb.slice(0, 3).map((v) => v >> 4).join(',');
  const counts = new Map<string, number>();
  for (const rgb of samples) counts.set(key(rgb), (counts.get(key(rgb)) ?? 0) + 1);
  const common = [...counts].sort((a, b) => b[1] - a[1])[0][0];
  const background = samples.find((rgb) => key(rgb) === common)!;
  return samples.filter((rgb) => apart(rgb, background));
}

async function selectedTakesTheAccent(t: Fixture) {
  const tabs = await t.app.chrome.elements({ scope: 'tabBar' }).then(
    (items) => items.filter((item) => item.role === 'tab'),
    () => null,
  );
  if (!tabs?.length) return t.skip('the host does not report its tab items');
  const window = await t.app.chrome.viewport();
  const selected = tabs.find((tab) => tab.selected);
  expect(selected, `a selected tab among ${tabs.map((tab) => tab.label).join(', ')}`).toBeDefined();
  const [, top, , height] = selected!.bounds;
  // A desktop sidebar is product chrome on the shell palette, not the page's.
  if (top + height < window.top + window.height * 0.5) t.skip('no tab bar on the bottom edge');
  // The accent as this page's theme draws it: a primary button's fill. The View tab shows the
  // group once its Logic has read the start query.
  await expect(t.app.view.testId(rootOf('dialog'))).toBeVisible();
  const accent = await fillOf(t, t.app.view.testId('lab-overlay-confirm'));

  const shot = await decodePng((await rawAutomation().lxapps.screenshot()).base64);
  const scale = shot.width / window.width;
  const ink = inkOf(shot, scale, window, selected!.bounds);
  const tinted = ink.filter((rgb) => sameHue(rgb, accent));
  console.log(`accent ${accent}; selected ${selected!.label}: ${tinted.length}/${ink.length} ink in its hue`);
  expect(ink.length, 'the selected tab draws an icon and label').toBeGreaterThan(3);
  expect(tinted.length * 2, `selected ink ${ink.slice(0, 4).join(' ')} takes the accent ${accent}`).toBeGreaterThan(ink.length);
  for (const tab of tabs.filter((tab) => !tab.selected)) {
    const other = inkOf(shot, scale, window, tab.bounds);
    const accented = other.filter((rgb) => sameHue(rgb, accent));
    expect(accented.length * 2, `${tab.label} is not drawn in the accent: ${accented.slice(0, 4).join(' ')}`).toBeLessThanOrEqual(other.length);
  }
}

// The selected tab is drawn in the page theme's accent; the others are not.
test('the selected tab takes the page theme accent', {
  id: 'TABBAR-COLOUR-001', covers: ['app:tabBar.style'],
  start: openUnderTabs('dialog'),
}, selectedTakesTheAccent);
