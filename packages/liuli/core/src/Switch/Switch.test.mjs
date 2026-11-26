import { test } from 'node:test';
import assert from 'node:assert/strict';
import { described, renderExample, renderPage, isSpinner } from '../testing/harness.mjs';

test('Basic: named by its label, described by its help text, and a toggle reaches the bound cell', async () => {
  const screen = await renderExample('Switch/Switch', 'Basic');
  const control = screen.getByTestId('sync');
  assert.equal(control.tag, 'switch');
  assert.equal(control.props.accessibilityLabel, 'Sync over mobile data');
  assert.deepEqual(described(screen, control), ['Photos wait for Wi-Fi either way.']);
  assert.ok(JSON.stringify(control.children).includes('Photos wait for Wi-Fi either way.'), 'the help text is inside the press target');
  assert.equal(control.props.checked, 1);
  screen.toggle(control, false);
  assert.equal(screen.getByTestId('sync').props.checked, 0);
});

test('InList: each row is its own switch', async () => {
  const screen = await renderExample('Switch/Switch', 'InList');
  screen.toggle(screen.getByTestId('digest'), true);
  assert.equal(screen.getByTestId('digest').props.checked, 1);
  assert.equal(screen.getByTestId('mentions').props.checked, 1);
});

test('the track turns to the deepened accent when on, and the thumb rings on keyboard focus', async () => {
  const screen = await renderExample('Switch/Switch', 'Basic');
  const control = screen.getByTestId('sync');
  // The on track is a checked state style, so the View paints it from the switch's own state.
  assert.match(control.props.checkedStyle.trackStyle, /"accent-solid"/);
  assert.match(control.props.trackStyle, /"line-strong\/45"/);
  screen.toggle(control, false);
  assert.equal(screen.getByTestId('sync').props.checked, 0);
  assert.match(control.props.focusVisibleStyle.thumbStyle, /focus/);
});

test('a row rings keyboard focus inside its edges, not round the thumb', async () => {
  const screen = await renderExample('Switch/Switch', 'InList');
  const focus = screen.getByTestId('mentions').props.focusVisibleStyle;
  assert.deepEqual(JSON.parse(focus.ring), { inner: { width: 2, color: 'focus' } });
  assert.equal(focus.thumbStyle, undefined);
});

test('Applying: busy, takes no press, and shows a spinner beside the words', async () => {
  const screen = await renderExample('Switch/Switch', 'Applying');
  const control = screen.getByTestId('applying');
  assert.deepEqual([control.props.stateBusy, control.props.enabled, control.props.checked], [1, 0, 1]);
  assert.equal(screen.queryAll(isSpinner).length, 1);
});

test('LabelHidden: named by its label, which is not drawn', async () => {
  const screen = await renderExample('Switch/Switch', 'LabelHidden');
  const control = screen.getByTestId('pin');
  assert.equal(control.props.accessibilityLabel, 'Pin chat');
  assert.equal(JSON.stringify(control.children ?? []).includes('Pin chat'), false);
  screen.toggle(control, true);
  assert.equal(screen.getByTestId('pin').props.checked, 1);
});

const savedPage = component => `
  import { useLocal } from '@lingxia/view';
  import { ${component} } from '@liulidesign/core/${component}';
  let finish: (ok: boolean) => void = () => {};
  let calls = 0;
  const action = () => { calls++; return new Promise<void>((resolve, reject) => { finish = ok => (ok ? resolve() : reject(Error('offline'))); }); };
  export function render() {
    const on = useLocal('on', false);
    return <page>
      <${component} label="Share usage data" description="Saved to your account." bind={on} changeAction={action} data-testid="saved" />
      <button data-testid="ok" onClick={() => finish(true)}>Ok</button>
      <button data-testid="fail" onClick={() => finish(false)}>Fail</button>
      <text data-testid="calls">{String(calls)}</text>
    </page>;
  }
`;

test('changeAction: flips at once, busy but not disabled, holds further presses, and keeps the value on success', async () => {
  const screen = await renderPage(savedPage('Switch'));
  screen.toggle(screen.getByTestId('saved'), true);
  const busy = screen.getByTestId('saved');
  assert.deepEqual([busy.props.checked, busy.props.stateBusy, busy.props.enabled], [1, 1, undefined]);
  assert.equal(screen.queryAll(isSpinner).length, 0, 'no spinner: the row does not reflow');
  assert.throws(() => screen.toggle(screen.getByTestId('saved'), false), /did not update value/, 'a press while saving is held off');
  assert.equal(screen.getByTestId('saved').props.checked, 1);
  screen.press(screen.getByTestId('ok'));
  await new Promise(resolve => setImmediate(resolve));
  screen.rerender({});
  const done = screen.getByTestId('saved');
  assert.deepEqual([done.props.checked, done.props.stateBusy], [1, undefined]);
  assert.equal(screen.getByText('1').tag, 'text', 'the action ran once');
});

test('changeAction: a failure flips back and is said under the label, as a description, until the next change', async () => {
  const screen = await renderPage(savedPage('Switch'));
  const errors = [];
  const original = console.error;
  console.error = (...args) => errors.push(args.join(' '));
  try {
    screen.toggle(screen.getByTestId('saved'), true);
    screen.press(screen.getByTestId('fail'));
    await new Promise(resolve => setImmediate(resolve));
    screen.rerender({});
  } finally {
    console.error = original;
  }
  const control = screen.getByTestId('saved');
  assert.deepEqual([control.props.checked, control.props.stateBusy], [0, undefined]);
  assert.deepEqual(described(screen, control), ['Saved to your account.', "That didn't work. Try again."]);
  screen.toggle(screen.getByTestId('saved'), true);
  assert.deepEqual(described(screen, screen.getByTestId('saved')), ['Saved to your account.']);
});

const quietly = async run => {
  const original = console.error;
  console.error = () => {};
  try { await run(); } finally { console.error = original; }
};

test('SavedLabelHidden alone: the failure is an alert, describes the switch, and is marked before the track', async () => {
  const screen = await renderExample('Switch/Switch', 'SavedLabelHidden');
  await quietly(async () => {
    screen.toggle(screen.getByTestId('pin-saved'), true);
    await new Promise(resolve => setImmediate(resolve));
    screen.rerender({});
  });
  const control = screen.getByTestId('pin-saved');
  assert.equal(control.props.checked, 0);
  assert.deepEqual(described(screen, control), ["That didn't work. Try again."]);
  const mark = screen.getByTestId('pin-saved-failed');
  assert.deepEqual([mark.props.role, mark.props.accessibilityLabel], ['img', "That didn't work. Try again."]);
  assert.equal(screen.queryAll(node => node.props.role === 'alert').length, 2, 'each label-hidden switch with an action keeps its alert mounted');
  assert.equal(screen.queryAll(node => node.props.role === 'alert' && node.children.some(child => child.text === "That didn't work. Try again.")).length, 1, 'only the failed one says it');
});

test('SavedLabelHidden in an Item: the row draws the failure under its words, quietly; the switch still reads it', async () => {
  const screen = await renderExample('Switch/Switch', 'SavedLabelHidden');
  await quietly(async () => {
    screen.toggle(screen.getByTestId('backup-saved'), true);
    await new Promise(resolve => setImmediate(resolve));
    screen.rerender({});
  });
  const control = screen.getByTestId('backup-saved');
  assert.deepEqual(described(screen, control), ["That didn't work. Try again."]);
  assert.throws(() => screen.getByTestId('backup-saved-failed'), 'no mark beside it in a row');
  const said = screen.queryAll(node => node.text === "That didn't work. Try again.");
  assert.equal(said.length, 2, 'once in the switch\'s alert, once drawn in the row');
});

test('label hidden: the mark and the row\'s line go with the next change', async () => {
  const screen = await renderPage(`
    import { useLocal } from '@lingxia/view';
    import { Switch } from '@liulidesign/core/Switch';
    import { Item } from '@liulidesign/core/Item';
    let fail = true;
    const action = async () => { if (fail) throw Error('offline'); };
    export function render() {
      const a = useLocal('a', false);
      const b = useLocal('b', false);
      return <page>
        <Switch label="Pin" isLabelHidden bind={a} changeAction={action} data-testid="alone" />
        <Item label="Back up" endContent={<Switch label="Back up" isLabelHidden bind={b} changeAction={action} data-testid="inrow" />} />
        <button data-testid="heal" onClick={() => { fail = false; }}>Heal</button>
      </page>;
    }
  `);
  const settle = async () => { await new Promise(resolve => setImmediate(resolve)); screen.rerender({}); };
  await quietly(async () => {
    screen.toggle(screen.getByTestId('alone'), true);
    screen.toggle(screen.getByTestId('inrow'), true);
    await settle();
  });
  screen.getByTestId('alone-failed');
  assert.equal(screen.queryAll(node => node.text === "That didn't work. Try again.").length, 3, 'two alerts and the row\'s line');
  screen.press(screen.getByTestId('heal'));
  await settle();
  // The mark goes before the switch, so the harness's positional re-lookup in toggle() would miss it: fire and look again by id.
  screen.fire(screen.getByTestId('alone'), 'onCheckedChange');
  screen.toggle(screen.getByTestId('inrow'), true);
  await settle();
  assert.equal(screen.getByTestId('alone').props.checked, 1);
  assert.throws(() => screen.getByTestId('alone-failed'));
  assert.equal(screen.queryAll(node => node.text === "That didn't work. Try again.").length, 0);
});

test('a compact DensityProvider makes the switch small unless it names a size; a disabled switch says why', async () => {
  const screen = await renderPage(`
    import { DensityProvider } from '@liulidesign/core/density';
    import { Switch } from '@liulidesign/core/Switch';
    export function render() {
      return (
        <page>
          <Switch label="Plain" value={false} onChange={() => {}} data-testid="plain" />
          <Switch label="Small" value={false} onChange={() => {}} size="sm" data-testid="small" />
          <DensityProvider value="compact">
            <view>
              <Switch label="Dense" value={false} onChange={() => {}} data-testid="dense" />
              <Switch label="Named" value={false} onChange={() => {}} size="md" data-testid="named" />
            </view>
          </DensityProvider>
          <Switch label="Locked" description="Sync between devices." value={false} onChange={() => {}} isDisabled disabledMessage="Sign in to sync." data-testid="locked" />
        </page>
      );
    }`);
  const track = id => screen.getByTestId(id).props.trackStyle;
  assert.equal(track('dense'), track('small'));
  assert.equal(track('named'), track('plain'));
  assert.notEqual(track('small'), track('plain'));
  assert.deepEqual(described(screen, screen.getByTestId('locked')), ['Sign in to sync.']);
});
