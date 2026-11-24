import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample, renderPage } from '../testing/harness.mjs';

const texts = (screen, root) => screen.queryAll(node => node.tag === 'text' && node.props.accessibilityLabel !== undefined);

test('People: a photo is named by alt; without one, the initials are a picture named by the full name; a failed photo falls back', async () => {
  const screen = await renderExample('Avatar/Avatar', 'People');
  assert.equal(screen.queryAll(node => node.tag === 'image' && node.props.imageAlt === 'Ana Lima').length, 1);
  screen.getByText('KC');
  const kai = texts(screen).find(node => node.props.accessibilityLabel === 'Kai Chen');
  assert.ok(kai);
  assert.equal(kai.props.role, 'img');
  const mira = screen.queryAll(node => node.tag === 'image' && node.props.imageAlt === 'Mira Patel')[0];
  screen.image(mira, 'error');
  assert.equal(screen.queryAll(node => node.tag === 'image' && node.props.imageAlt === 'Mira Patel').length, 0);
  assert.ok(texts(screen).some(node => node.props.accessibilityLabel === 'Mira Patel'));
});

test('Group: a named group showing four, the rest as a count with the app\'s words', async () => {
  const screen = await renderExample('Avatar/Avatar', 'Group');
  assert.equal(screen.getByTestId('editors').props.accessibilityLabel, 'Editors');
  const more = screen.getByText('+2');
  assert.equal(more.props.accessibilityLabel, '2 more editors');
});

test('WithName: a decorative avatar is hidden from assistive tech', async () => {
  const screen = await renderExample('Avatar/Avatar', 'WithName');
  assert.equal(screen.queryAll(node => node.props.accessibilityHidden === 1 && node.tag === 'column').length >= 1, true);
});

test('Presence: the status is a dot that is not read; its words follow the name; offline is hollow', async () => {
  const screen = await renderExample('Avatar/Avatar', 'Presence');
  const dot = id => screen.getByTestId(id).children.find(node => node.props.position === 'absolute' && node.tag !== 'image' && node.props.accessibilityHidden === 1);
  assert.deepEqual(['online', 'away', 'busy'].map(id => dot(id).props.background), ['success-mark', 'warning-mark', 'danger-mark']);
  assert.deepEqual([dot('offline').props.background, Boolean(dot('offline').props.ring)], ['surface', true]);
  assert.deepEqual([dot('online').props.width, dot('online').props.borderColor], [12, 'surface'], 'cut out of the avatar by the surface colour');
  const named = id => screen.getByTestId(id).children.map(node => node.props.accessibilityLabel ?? node.props.alt).filter(Boolean);
  assert.deepEqual(named('away'), ['Kai Chen, Away']);
  assert.ok(screen.getByTestId('online').children.some(node => Object.values(node.props).includes('Ana Lima, Online')), 'a photo is named the same way');
});

test('Unread: a badge sits on the top end edge and is read by its own label', async () => {
  const screen = await renderExample('Avatar/Avatar', 'Unread');
  const slot = screen.getByTestId('unread').children.at(-1);
  assert.equal(slot.props.position, 'absolute');
  assert.equal(slot.props.accessibilityHidden, undefined);
  screen.getByText('3');
});

test('Teams: a rounded square for a team, its corner a quarter of its size', async () => {
  const team = (await renderExample('Avatar/Avatar', 'Teams')).getByTestId('team');
  assert.deepEqual([team.props.width, team.props.height, team.props.radius], [40, 40, 10]);
});

test('At large text: one or two initials and a photo keep one square box, grown up to 1.3 times; the initials stay a share of it', async () => {
  for (const [textScale, side, letters] of [[1, 40, undefined], [1.3, 52, 13], [2, 52, 8.5]]) {
    const screen = await renderPage(`
      import { Avatar } from '@liulidesign/core/Avatar';
      export function render() { return <page><Avatar name="Lena" data-testid="one" /><Avatar name="Alexandra Montgomery" data-testid="two" /><Avatar name="Kai Chen" src="kai.png" data-testid="photo" /></page>; }
    `, { env: { textScale } });
    for (const id of ['one', 'two', 'photo']) {
      const { props } = screen.getByTestId(id);
      assert.deepEqual([props.width, props.height], [side, side], `${id} at ${textScale}`);
      assert.equal(props.widthKeyword, undefined, 'never fit to its initials, which would make an oval');
    }
    const initials = screen.getByTestId('two').children[0];
    assert.equal(initials.props.fontSize, letters ?? 13, `initials at ${textScale}`);
  }
});

// The largest text sizes as the hosts report them, [authored, shown]: iOS Dynamic Type at AX5 scales every size by body's
// 53 / 17; Android 14's nonlinear table at font scale 2 grows small sizes twice and body text far less.
const hostCurves = {
  'iOS AX5': [[0, 0], [100, 100 * 53 / 17]],
  'Android 2.0': [[0, 0], [8, 16], [10, 20], [12, 24], [14, 26], [18, 30], [20, 34], [24, 36], [30, 38], [100, 100]],
};
const shown = (curve, size) => {
  const at = curve.findIndex(([x]) => x >= size);
  const [[x0, y0], [x1, y1]] = [curve[at - 1], curve[at]];
  return y0 + (y1 - y0) * (size - x0) / (x1 - x0);
};

test('At the hosts\' largest text sizes, two initials still fit inside the disc at every size', async () => {
  for (const [host, curve] of Object.entries(hostCurves)) {
    const textScale = Math.round(shown(curve, 17) / 17 * 100) / 100;
    const screen = await renderPage(`
      import { Avatar } from '@liulidesign/core/Avatar';
      export function render() { return <page>${['xs', 'sm', 'md', 'lg'].map(size => `<Avatar name="Alexandra Montgomery" size="${size}" data-testid="${size}" />`).join('')}</page>; }
    `, { env: { textScale } });
    for (const size of ['xs', 'sm', 'md', 'lg']) {
      const avatar = screen.getByTestId(size);
      const letters = shown(curve, avatar.children[0].props.fontSize);
      // Two semibold capitals take a box about 1.4 em wide and 0.75 em tall; its corners stay inside the circle.
      const reach = Math.hypot(letters * 1.4 / 2, letters * 0.75 / 2);
      assert.ok(reach <= avatar.props.width / 2, `${host}, ${size}: ${letters.toFixed(1)} pt initials reach ${reach.toFixed(1)} pt in a ${avatar.props.width} pt disc`);
      assert.ok(letters >= 11, `${host}, ${size}: initials stay legible`);
    }
  }
});

test('At large text: the group\'s count is the avatars\' size', async () => {
  const screen = await renderExample('Avatar/Avatar', 'Group', { env: { textScale: 2 } });
  const overflow = screen.getByTestId('editors-overflow');
  assert.deepEqual([overflow.props.width, overflow.props.height], [52, 52]);
});

test('Group: the count takes the group\'s shape, a circle like the avatars beside it', async () => {
  const screen = await renderExample('Avatar/Avatar', 'Group');
  const group = screen.getByTestId('editors');
  const radii = [group.children[0].children[0], screen.getByTestId('editors-overflow')].map(node => node.props.radius);
  assert.deepEqual(radii, [16384, 16384]);
});

test('Hues: a name keeps one hue; the initials sit in the text role on its fill', async () => {
  const { hueFor } = await import('../hue.ts');
  assert.equal(hueFor('Ana Lima'), hueFor('Ana Lima'));
  const seen = new Set(['Ana Lima', 'Kai Chen', 'Mira Patel', 'Tomas Reyes', 'Yuki Sato', 'Noor Haddad'].map(hueFor));
  assert.ok(seen.size >= 4, `six names spread over ${seen.size} hues`);
  const screen = await renderExample('Avatar/Avatar', 'Hues');
  const ana = screen.getByTestId('hue-ana');
  assert.equal(ana.props.background, `hue-${hueFor('Ana Lima')}-soft`);
  assert.equal(ana.children[0].props.color, 'text');
});

test('People: a photo keeps the box its exact size; only initials grow it', async () => {
  const screen = await renderExample('Avatar/Avatar', 'People');
  const ana = screen.getByTestId('ana');
  assert.deepEqual([ana.props.width, ana.props.height, ana.props.minWidth], [40, 40, undefined]);
});
