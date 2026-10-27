import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, describe, prepare, commit, discard, dispatch, useLocal, useView, createView, readsHostContext } from '../src/internal.js';
import { decode, rendered } from './support/wire.mjs';

const throwsOn = (render, pattern) => assert.throws(() => describe(render, {}), pattern);

test('image lifecycle callbacks are committed handles and retry keys are bounded', () => {
  let loads = 0, errors = 0;
  const render = () => h('image', { src: 'photo.png', retryKey: 'attempt-1', onLoad: () => loads++, onError: () => errors++ });
  const { props } = rendered(render);
  assert.equal(props.imageRetryKey, 'attempt-1');
  prepare(render, {}); discard();
  assert.equal(dispatch(props.onImageLoad), true); assert.equal(loads, 1);
  assert.equal(dispatch(props.onImageError), true); assert.equal(errors, 1);
  describe(render, {}); assert.equal(dispatch(props.onImageLoad), false);
  for (const retryKey of [3, 'x'.repeat(257), '\0']) throwsOn(() => h('image', { src: 'photo.png', retryKey }), /UI_IMAGE/);
  throwsOn(() => h('image', { src: 'photo.png', onError: true }), /UI_IMAGE/);
});

test('image alternative text distinguishes descriptions from decorative images and bounds UTF-8', () => {
  for (const alt of ['', 'A mountain lake', '图片'.repeat(170)]) assert.equal(rendered(() => h('image', { src: 'photo.png', alt })).props.imageAlt, alt);
  for (const alt of [null, 1, true, {}, '   ', '\0', '图片'.repeat(171), '\ud800']) throwsOn(() => h('image', { src: 'photo.png', alt }), /UI_IMAGE/);
  throwsOn(() => h('text', { alt: 'Wrong primitive' }, 'Text'), /UI_PROP/);
});

test('image sources and fits are validated; objectFit is image and vector style', () => {
  const image = rendered(() => h('image', { src: 'assets/avatar.png', style: { width: 64, height: 64, objectFit: 'contain' } }));
  assert.deepEqual([image.tag, image.props.imageSrc, image.props.imageFit], ['image', 'assets/avatar.png', 'contain']);
  for (const props of [{}, { src: 42 }, { src: '' }, { src: 'a\0b' }]) throwsOn(() => h('image', props), /UI_IMAGE/);
  throwsOn(() => h('image', { src: 'ok', style: { objectFit: 'stretch' } }), /UI_STYLE_VALUE: objectFit/);
  throwsOn(() => h('image', { src: 'a' }, h('text', {}, 'x')), /UI_CHILD/);
  throwsOn(() => h('column', { src: 'a' }), /UI_PROP/);
  throwsOn(() => h('text', { style: { objectFit: 'cover' } }, 'x'), /UI_STYLE: objectFit is not supported on <text>/);
});

test('objectPosition lowers CSS keywords and percentages to box fractions on images only', () => {
  const at = position => rendered(() => h('image', { src: 'a', style: { objectPosition: position } })).props.imagePosition;
  assert.equal(at('center'), '[0.5,0.5]');
  assert.equal(at('top'), '[0.5,0]');
  assert.equal(at('right'), '[1,0.5]');
  assert.equal(at('bottom left'), '[0,1]');
  assert.equal(at('center top'), '[0.5,0]');
  assert.equal(at('25% 75%'), '[0.25,0.75]');
  for (const bad of ['10px 0', 'left right', 'top top', 'a b c', 42]) throwsOn(() => h('image', { src: 'a', style: { objectPosition: bad } }), /UI_STYLE_VALUE: objectPosition/);
  throwsOn(() => h('vector', { paths: [{ d: 'M0 0H1', stroke: '#000000' }], style: { objectPosition: 'top' } }), /UI_STYLE: objectPosition is not supported on <vector>/);
});

test('vector paths carry SVG transforms, caps, joins, dash offsets and named gradients', () => {
  const gradients = { sweep: { type: 'conic', from: 45, stops: [{ offset: 0, color: '#000000' }, { offset: 1, color: '#ffffff' }] }, glow: { type: 'radial', r: 0.5, stops: [{ offset: 0, color: '#ffffff' }, { offset: 1, color: '#ff0000', opacity: 0.5 }] } };
  const paths = [{ d: 'M2 12h20', fill: 'url(#glow)', stroke: 'url(#glow)', strokeLinecap: 'round', strokeLinejoin: 'bevel', strokeDashoffset: 2, strokeDasharray: [4, 2], transform: 'rotate(45 12 12) translate(1,2)' }];
  const source = JSON.parse(rendered(() => h('vector', { paths, gradients })).props.vectorSource);
  assert.deepEqual(source, { viewBox: [0, 0, 24, 24], paths, gradients });
  for (const [path, extra] of [
    [{ d: 'M0 0', fill: 'url(#missing)' }, {}], [{ d: 'M0 0', strokeLinecap: 'arrow' }, {}], [{ d: 'M0 0', strokeLinejoin: 'arcs' }, {}],
    [{ d: 'M0 0', transform: 'perspective(1)' }, {}], [{ d: 'M0 0', strokeDashoffset: Infinity }, {}],
    [{ d: 'M0 0' }, { gradients: { 'bad id': gradients.glow } }], [{ d: 'M0 0' }, { gradients: { g: { type: 'conic', stops: [] } } }],
    [{ d: 'M0 0' }, { gradients: { g: { type: 'linear', r: 1, stops: [{ offset: 0, color: '#000000' }] } } }],
    [{ d: 'M0 0' }, { gradients: Object.fromEntries(Array.from({ length: 9 }, (_, i) => [`g${i}`, gradients.glow])) }],
  ]) throwsOn(() => h('vector', { paths: [path], ...extra }), /UI_VECTOR/);
});

test('vectors encode bounded path data, fit and dash patterns, and decorate composed buttons', () => {
  const paths = [{ d: 'M2 2h20v20H2z', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5 }];
  const vector = rendered(() => h('vector', { paths }));
  assert.equal(vector.tag, 'vector');
  assert.deepEqual(JSON.parse(vector.props.vectorSource), { viewBox: [0, 0, 24, 24], paths });
  assert.doesNotThrow(() => describe(() => h('button', { accessibilityLabel: 'Open' }, h('vector', { paths })), {}));
  for (const props of [{ paths: [] }, { paths: [{ d: '' }] }, { paths: [{ d: 'M0 0', href: 'remote' }] }, { paths, viewBox: [0, 0, 0, 24] },
    { paths: [{ d: 'M0 0', strokeWidth: Infinity }] }, { paths: [{ d: 'M0 0', fill: 'url(remote)' }] }])
    throwsOn(() => h('vector', props), /UI_VECTOR/);
  throwsOn(() => h('vector', { paths }, h('text', {}, 'child')), /UI_CHILD/);
  throwsOn(() => h('vector', { paths, onClick() {} }), /UI_PROP/);
  const stroke = { d: 'M0 .5H1', fill: 'none', stroke: '#000000', vectorEffect: 'non-scaling-stroke', strokeDasharray: [4, 2, 1] };
  const fitted = JSON.parse(rendered(() => h('vector', { paths: [stroke], style: { objectFit: 'fill' } })).props.vectorSource);
  assert.equal(fitted.fit, 'fill'); assert.deepEqual(fitted.paths[0].strokeDasharray, [4, 2, 1]);
  throwsOn(() => h('vector', { paths, style: { objectFit: 'cover' } }), /UI_STYLE_VALUE/);
  for (const strokeDasharray of [[0, 2], [-1], [1 / 128], [1025], [Infinity], [NaN], '4 2', Array(17).fill(1)])
    throwsOn(() => h('vector', { paths: [{ ...stroke, strokeDasharray }] }), /UI_VECTOR/);
});

test('property admission stays local to each element and rejects prototype names', () => {
  for (const [tag, property] of [
    ['text', 'onClick'], ['button', 'onInput'], ['column', 'disabled'], ['row', 'row'], ['scroll', 'count'], ['list', 'onClick'],
    ['image', 'columns'], ['grid', 'src'], ['plot', 'onChange'], ['input', 'onClick'], ['textarea', 'onChange'], ['search', 'options'],
    ['checkbox', 'min'], ['switch', 'placeholder'], ['slider', 'secure'], ['radio', 'step'], ['spacer', 'lines'], ['stack', 'onReachStart'],
    ['text', 'lines'], ['grid', 'columns'], ['image', 'fit'], ['button', 'enabled'],
  ]) {
    throwsOn(() => h(tag, { [property]: 1 }), new RegExp(`UI_PROP: ${property} is not a property of <${tag}>`));
    for (const key of ['constructor', 'toString', '__proto__']) throwsOn(() => h(tag, { [key]: 1 }), /UI_PROP:/);
  }
});

test('children flatten; booleans, nullish values and empty strings render nothing', () => {
  const root = rendered(() => h('column', {}, false, null, '', [h('text', { testId: 'hello' }, 'Hello ', 42)], '' && h('text', {}, 'never')));
  assert.equal(root.children.length, 1);
  assert.deepEqual([root.children[0].testId, root.children[0].text], ['hello', 'Hello 42']);
});

test('view is a column; row is the horizontal container', () => {
  assert.equal(rendered(() => h('view', {})).tag, 'column');
  assert.equal(rendered(() => h('row', {})).tag, 'row');
});

test('components receive props only and read the View through useView', () => {
  const view = createView({ greeting: 'Hello' }, {});
  let args;
  function Greeting(...received) { args = received.length; return h('text', {}, received[0].prefix, useView().data.greeting); }
  assert.equal(rendered(() => h(Greeting, { prefix: '> ' }), view).text, '> Hello');
  assert.equal(args, 1);
});
test('unsupported tags, callbacks and child shapes fail with their JSX site', () => {
  for (const render of [() => h('div'), () => h('toString'), () => h('button', { onClick: 42 }), () => h('text', {}, {}), () => h('spacer', {}, 'x'), () => [h('text')]])
    assert.throws(() => describe(render, {}), /UI_/);
  const Card = () => h('column', { __loc: 'pages/home.tsx:4:10' }, h('text', { __loc: 'pages/home.tsx:5:12', onClick() {} }, 'x'));
  assert.throws(() => describe(() => h('column', {}, h(Card, { __loc: 'pages/home.tsx:9:3' })), {}),
    /UI_PROP: onClick is not a property of <text>\n  at <text> \(pages\/home.tsx:5:12\)\n  at <column> \(pages\/home.tsx:4:10\)\n  at Card \(pages\/home.tsx:9:3\)/);
});

test('scroll retains child button callbacks and rejects unsupported scroll props', () => {
  const scroll = rendered(() => h('scroll', { testId: 'feed', style: { height: 100 } }, h('button', { key: 'item', onClick() {} }, 'Item')));
  assert.deepEqual([scroll.tag, scroll.testId, scroll.children[0].tag], ['scroll', 'feed', 'button']);
  assert.equal(typeof scroll.children[0].props.onClick, 'number');
  throwsOn(() => h('scroll', { onClick() {} }), /UI_PROP/);
});

const buttons = buffer => decode(buffer).filter(node => node.tag === 'button').map(node => ({ text: node.text, handle: node.props.onClick }));

test('keyed components retain local state across reorder and release it on removal', () => {
  function Counter({ name }) {
    const count = useLocal('count', 0);
    return h('button', { onClick: () => count.set(count.value + 1) }, `${name}:${count.value}`);
  }
  let names = ['a', 'b'];
  const render = () => h('column', {}, names.map(name => h(Counter, { key: name, name })));
  const nodes = buttons(describe(render, {}));
  assert.equal(dispatch(nodes[0].handle), true);
  names = ['b', 'a'];
  assert.deepEqual(buttons(describe(render, {})).map(n => n.text), ['b:0', 'a:1']);
  assert.equal(dispatch(nodes[0].handle), false);
  names = ['b']; describe(render, {});
  names = ['a', 'b'];
  assert.equal(buttons(describe(render, {}))[0].text, 'a:0');
});

test('discarded candidates preserve committed callbacks and local state', () => {
  const render = () => { const count = useLocal('count', 0); return h('button', { onClick: () => count.set(count.value + 1) }, count.value); };
  const first = buttons(describe(render, {}))[0];
  dispatch(first.handle);
  assert.equal(buttons(prepare(render, {}))[0].text, '1');
  discard();
  assert.equal(dispatch(first.handle), true);
  assert.equal(buttons(prepare(render, {}))[0].text, '1');
  commit();
  assert.equal(dispatch(first.handle), false);
});

test('a set on a prepared candidate cell applies when that candidate commits', () => {
  let cell;
  const render = () => { cell = useLocal('fresh', 'initial'); return h('text', {}, cell.value); };
  prepare(render, {});
  cell.set('before commit');
  commit();
  assert.equal(rendered(render).text, 'before commit');
});

test('disabled controls and render-time mutations cannot dispatch', () => {
  const button = rendered(() => h('button', { disabled: true, onClick() { throw Error('called'); } }, 'Disabled'));
  assert.equal(button.props.onClick, undefined); assert.equal(button.props.enabled, 0);
  throwsOn(() => { useLocal('x', 0).set(1); return h('text'); }, /UI_RENDER/);
  throwsOn(() => h('button', { disabled: 'yes' }), /UI_PROP: disabled/);
});

test('replicated data and environment are deeply immutable without prototype mutation', () => {
  const data = JSON.parse('{"nested":{"value":1},"rows":[{"text":"first"}],"__proto__":{"injected":true}}');
  const view = createView(data, { viewport: { width: 300, height: 500 } });
  assert.throws(() => { view.data.nested.value = 2; }, TypeError);
  assert.throws(() => { view.data.rows[0].text = 'changed'; }, TypeError);
  assert.throws(() => { view.data.rows.push({ text: 'new' }); }, TypeError);
  assert.throws(() => { view.env.viewport.width = 1; }, TypeError);
  assert.equal(Object.getPrototypeOf(view.data), Object.prototype);
  assert.equal(view.data.injected, undefined);
  assert.equal(Object.hasOwn(view.data, '__proto__'), true);
});

test('a retained candidate cannot alias a new render arena', () => {
  let old;
  describe(() => old = h('text', {}, 'old'), {});
  throwsOn(() => { h('text', {}, 'new'); return old; }, /UI_STALE/);
  assert.equal(rendered(() => h('text', {}, 'recovered')).text, 'recovered');
});

test('boolean controls dispatch the inverse controlled value and expire old handles', () => {
  for (const tag of ['checkbox', 'switch']) {
    let received;
    const render = () => h(tag, { value: true, onChange: value => { received = value; } }, 'Preference');
    const first = rendered(render);
    assert.equal(first.text, 'Preference'); assert.equal(first.props.checked, 1);
    assert.equal(dispatch(first.props.onCheckedChange), true); assert.equal(received, false);
    assert.equal(rendered(render).props.checked, 1);
    assert.equal(dispatch(first.props.onCheckedChange), false);
    assert.equal(rendered(() => h(tag, { value: true, disabled: true, onChange() { throw Error('disabled'); } })).props.onCheckedChange, undefined);
  }
});

test('boolean control bindings persist and reject invalid values or children', () => {
  const render = () => h('switch', { bind: useLocal('preference', false), accessibilityLabel: 'Sync' }, 'Preference');
  const first = rendered(render);
  assert.equal(first.props.checked, 0); assert.equal(first.props.accessibilityLabel, 'Sync');
  assert.equal(dispatch(first.props.onCheckedChange), true);
  assert.equal(rendered(render).props.checked, 1);
  for (const tag of ['checkbox', 'switch']) {
    for (const props of [{ value: 1 }, { value: 'true' }, { onChange: 2 }, { bind: { value: false, set() {} } }, { accessibilityLabel: ' ' }])
      throwsOn(() => h(tag, props), /UI_/);
    for (const value of [undefined, null, 0, 'false']) throwsOn(() => h(tag, { bind: useLocal('invalid', value) }), /UI_/);
    throwsOn(() => h(tag, { bind: useLocal('valid', false), value: false }), /UI_/);
  }
  throwsOn(() => h('switch', {}, h('text', {}, 'Task')), /UI_CHILD/);
});

test('radio shares a string binding, retains selection across reorder, and ignores selecting the selected option', () => {
  let options = [{ value: 'daily', label: 'Daily' }, { value: 'weekly', label: 'Weekly' }, { value: 'never', label: 'Never', disabled: true }];
  const render = () => h('radio', { bind: useLocal('pace', 'daily'), options, accessibilityLabel: 'Pace' });
  let radio = rendered(render);
  assert.equal(radio.tag, 'radio'); assert.equal(radio.props.accessibilityLabel, 'Pace');
  assert.deepEqual(radio.children.map(r => r.props.checked), [1, 0, 0]);
  const stale = radio.children[1].props.onCheckedChange; assert.equal(dispatch(stale), true);
  options = [options[1], options[0], options[2]];
  radio = rendered(render);
  assert.deepEqual(radio.children.map(r => r.props.checked), [1, 0, 0]);
  assert.equal(radio.children[0].key, 'weekly');
  assert.equal(radio.children[2].props.onCheckedChange, undefined); assert.equal(radio.children[2].props.enabled, 0);
  assert.equal(dispatch(stale), false);
  assert.equal(dispatch(radio.children[0].props.onCheckedChange), true);
  assert.equal(rendered(render).children[0].props.checked, 1);
});

test('radio is controlled, validates options and binding, and disables the entire group', () => {
  const options = [{ value: 'a', label: 'A' }, { value: 'b', label: 'B' }]; const changed = [];
  const render = () => h('radio', { value: 'a', options, onChange: value => changed.push(value) });
  const radio = rendered(render);
  dispatch(radio.children[0].props.onCheckedChange); assert.deepEqual(changed, []);
  dispatch(radio.children[1].props.onCheckedChange); assert.deepEqual(changed, ['b']);
  const disabled = rendered(() => h('radio', { options, disabled: true, onChange() { throw Error('disabled'); } }));
  assert.ok(disabled.children.every(r => r.props.onCheckedChange === undefined && r.props.enabled === 0));
  for (const props of [{ options: [] }, { options: [options[0], options[0]] }, { options: Array.from({ length: 65 }, (_, i) => ({ value: String(i), label: 'x' })) },
    { options, value: 1 }, { options, onChange: 1 }, { options: [{ value: 'a', label: 1 }] }, { options: [{ value: 'a', label: 'A', enabled: false }] },
    { options, bind: { value: 'a', set() {} } }]) throwsOn(() => h('radio', props), /UI_/);
  throwsOn(() => h('radio', { options, bind: useLocal('bad', false) }), /UI_/);
  throwsOn(() => h('radio', { options, bind: useLocal('good', 'a'), value: 'a' }), /UI_/);
  throwsOn(() => h('radio-option', {}, 'forged'), /UI_TAG/);
  throwsOn(() => h('radio', { options }, h('text', {}, 'child')), /UI_CHILD/);
});

test('composed buttons take their accessible name from descendant text unless labeled', () => {
  let clicks = 0;
  const render = label => () => h('button', { accessibilityLabel: label, onClick: () => clicks++ },
    h('row', {}, h('text', {}, 'Inbox'), '3 unread'));
  const button = rendered(render(undefined));
  assert.equal(button.text, ''); assert.equal(button.props.accessibilityLabel, undefined);
  assert.deepEqual(button.children[0].children.map(child => child.text), ['Inbox', '3 unread']);
  assert.equal(dispatch(button.props.onClick), true); assert.equal(clicks, 1);
  assert.equal(rendered(render('Open inbox')).props.accessibilityLabel, 'Open inbox');
  for (const label of ['', ' ', 1, 'x\0y']) throwsOn(render(label), /UI_BUTTON/);
  const mixed = rendered(() => h('button', {}, 'plain', h('text', {}, 'child')));
  assert.deepEqual(mixed.children.map(child => [child.tag, child.text]), [['text', 'plain'], ['text', 'child']]);
});

test('composed controls hold controls of their own, never a scroll or list, and never just one control', () => {
  for (const tag of ['scroll', 'list']) {
    function Nested() { return h('column', {}, h(tag, {})); }
    throwsOn(() => h('button', {}, h(Nested, {})), /UI_BUTTON: a scroll or list cannot sit inside a pressable control/);
    throwsOn(() => h('checkbox', {}, h(Nested, {})), /UI_BUTTON/);
  }
  const row = rendered(() => h('button', { onClick() {} }, h('row', {}, h('text', {}, 'Wi-Fi'),
    h('switch', { value: true, onChange() {}, accessibilityLabel: 'Wi-Fi' }), h('button', { onClick() {} }, 'Delete'))));
  assert.deepEqual(row.children[0].children.map(child => child.tag), ['text', 'switch', 'button']);
  function Only() { return h('switch', { value: true, onChange() {}, accessibilityLabel: 'x' }); }
  throwsOn(() => h('button', { onClick() {} }, h(Only, {})), /UI_BUTTON: <button> holds only a <switch>; put the action on that control/);
  let checked;
  const box = rendered(() => h('checkbox', { value: false, onChange: value => checked = value }, h('row', {}, h('text', {}, 'Task'))));
  assert.equal(box.tag, 'checkbox'); assert.equal(box.text, '');
  assert.equal(dispatch(box.props.onCheckedChange), true); assert.equal(checked, true);
});

test('grid templates are a style; counts expand to equal tracks and any container may be a grid', () => {
  for (const gridTemplateColumns of [null, 0, -1, 1.5, 65, Infinity, '[a] 1fr', '1fr; x'])
    throwsOn(() => h('grid', { style: { gridTemplateColumns } }), /UI_(GRID|STYLE_VALUE)/);
  assert.doesNotThrow(() => describe(() => h('grid', {}, h('text', {}, 'one column')), {}));
  for (const columns of [1, 3, 64]) {
    const grid = rendered(() => h('grid', { style: { gridTemplateColumns: columns } }, h('text', { key: 'a' }, 'A'), h('text', { key: 'b' }, 'B')));
    assert.equal(grid.props.gridTemplateColumns, `repeat(${columns}, minmax(0, 1fr))`);
    assert.deepEqual(grid.children.map(child => child.key), ['a', 'b']);
  }
  const view = rendered(() => h('view', { style: { display: 'grid', gridTemplateColumns: '100px  1fr', gridAutoFlow: 'row dense', alignContent: 'space-between' } },
    h('text', { style: { gridColumn: 'span 2', gridColumnEnd: -1, gridRowStart: 2, justifySelf: 'end', order: -1 } }, 'A')));
  assert.deepEqual([view.props.gridTemplateColumns, view.props.gridAutoFlow, view.props.alignContent], ['100px 1fr', 'row-dense', 'between']);
  assert.deepEqual(view.children[0].props, { ...view.children[0].props, gridColumn: 'span 2 / -1', gridRow: '2', justifySelf: 'end', order: -1 });
  for (const style of [{ gridColumn: 'span 0' }, { gridRow: '1 / 2 / 3' }, { order: 1.5 }, { order: 20000 }])
    throwsOn(() => h('text', { style }, 'x'), /UI_STYLE_VALUE/);
  throwsOn(() => h('view', { style: { gridAutoFlow: 'dense' } }), /UI_STYLE_VALUE/);
  throwsOn(() => h('text', { style: { gridAutoFlow: 'row' } }, 'x'), /gridAutoFlow is not supported on <text>/);
});

test('text flattens nested spans and links into styled runs under one paragraph', () => {
  let pressed = 0;
  const tree = rendered(() => h('text', { style: { fontSize: 16 } }, 'Hello ',
    h('span', { style: { fontWeight: 700, color: 'danger' } }, 'bold ', h('span', { style: { fontStyle: 'italic' } }, 'both'), ' tail'),
    ' and ', h('link', { onPress: () => pressed++, accessibilityLabel: 'Terms' }, 'terms')));
  assert.equal(tree.text, '');
  assert.deepEqual(tree.children.map(child => [child.tag, child.text]),
    [['span', 'Hello '], ['span', 'bold '], ['span', 'both'], ['span', ' tail'], ['span', ' and '], ['link', 'terms']]);
  assert.deepEqual(tree.children[2].props, { fontWeight: 700, color: 'danger', fontStyle: 'italic' });
  assert.equal(tree.children[5].props.accessibilityLabel, 'Terms');
  assert.equal(typeof tree.children[5].props.onClick, 'number');
  assert.equal(rendered(() => h('text', {}, 'plain ', 'text')).text, 'plain text');
  throwsOn(() => h('column', {}, h('span', {}, 'loose')), /UI_TEXT: <span> belongs inside <text>/);
  assert.equal(rendered(() => h('button', {}, h('text', {}, 'Read ', h('link', { onPress() {} }, 'terms')))).tag, 'button');
  throwsOn(() => h('text', {}, h('link', {}, h('link', {}, 'x'))), /UI_TEXT: links do not nest/);
  const pieces = rendered(() => h('text', {}, h('link', { onPress() {}, accessibilityLabel: 'Terms' }, 'the ', h('span', { style: { fontWeight: 700 } }, 'terms')))).children;
  assert.deepEqual(pieces.map(piece => [piece.tag, piece.text, piece.props.fontWeight, piece.props.accessibilityLabel]), [['link', 'the ', undefined, 'Terms'], ['link', 'terms', 700, undefined]]);
  assert.equal(pieces[0].props.onClick, pieces[1].props.onClick);
  throwsOn(() => h('text', {}, h('span', { style: { padding: 4 } }, 'x')), /UI_STYLE: padding is not supported on <span>/);
  throwsOn(() => h('text', {}, Array.from({ length: 257 }, (_, i) => h('span', { key: String(i) }, 'x'))), /UI_TEXT: more than 256 runs/);
});

test('accessibility roles, heading levels, hidden subtrees and live regions are bounded props', () => {
  const tree = rendered(() => h('column', { role: 'list', accessibilityLabel: 'Bills' },
    h('text', { role: 'heading', headingLevel: 2 }, 'Title'), h('row', { role: 'listitem', accessibilityHidden: true }),
    h('text', { role: 'status', accessibilityLiveRegion: 'assertive' }, 'Saved')));
  assert.deepEqual([tree.props.role, tree.props.accessibilityLabel], ['list', 'Bills']);
  assert.deepEqual([tree.children[0].props.role, tree.children[0].props.headingLevel], ['heading', 2]);
  assert.equal(tree.children[1].props.accessibilityHidden, 1);
  assert.equal(tree.children[2].props.accessibilityLiveRegion, 'assertive');
  for (const props of [{ role: 'banner' }, { headingLevel: 2 }, { role: 'heading', headingLevel: 7 }, { accessibilityHidden: 'yes' },
    { accessibilityLiveRegion: 'rude' }, { accessibilityLabel: '  ' }])
    throwsOn(() => h('column', props), /UI_PROP/);
});

test('stack children position with insets', () => {
  const Child = () => h('button', { style: { position: 'absolute', left: -5, top: 12 } }, 'Top');
  const stack = rendered(() => h('stack', {}, h('text', {}, 'Back'), h(Child, {})));
  assert.equal(stack.tag, 'stack');
  assert.deepEqual([stack.children[1].props.position, stack.children[1].props.insetLeft, stack.children[1].props.insetTop], ['absolute', -5, 12]);
  throwsOn(() => h('stack', {}, h('text', { style: { offset: { x: 1 } } }, 'x')), /UI_STYLE: unknown style property offset/);
});

test('line clamps are text styles and preserve the source string', () => {
  const text = rendered(() => h('text', { style: { lineClamp: 2 } }, 'First\nSecond\nThird'));
  assert.equal(text.props.textLines, 2); assert.equal(text.text, 'First\nSecond\nThird');
  for (const lineClamp of [null, 0, -1, 1.5, 1025, Infinity, '2', true]) throwsOn(() => h('text', { style: { lineClamp } }, 'x'), /UI_STYLE_VALUE/);
  for (const tag of ['button', 'input', 'column', 'stack']) throwsOn(() => h(tag, { style: { lineClamp: 2 } }), /UI_STYLE: lineClamp is not supported/);
});


test('accessibilityState wires each declared state and refuses unknown keys or non-booleans', () => {
  const { props } = rendered(() => h('button', { onClick() {}, accessibilityState: { pressed: true, expanded: false } }, 'Bold'));
  assert.deepEqual([props.statePressed, props.stateExpanded, props.stateSelected], [1, 0, undefined]);
  for (const state of [{ checked: true }, { busy: 'yes' }, []])
    assert.throws(() => describe(() => h('column', { accessibilityState: state }), {}), /UI_PROP: accessibilityState/);
});

test('select sends its options and shows the chosen label or its placeholder', () => {
  const options = [{ value: 'm', label: 'Monthly' }, { value: 'y', label: 'Yearly', disabled: true }];
  const chosen = rendered(() => h('select', { options, value: 'm', accessibilityLabel: 'Plan', onChange() {} }));
  assert.equal(chosen.tag, 'select');
  assert.equal(chosen.text, 'Monthly');
  assert.equal(chosen.props.inputValue, 'm');
  assert.deepEqual(JSON.parse(chosen.props.selectOptions), options);
  assert.ok(chosen.props.onSelect > 0);
  const empty = rendered(() => h('select', { options, placeholder: 'Choose a plan', accessibilityLabel: 'Plan' }));
  assert.equal(empty.text, 'Choose a plan');
  assert.equal(empty.props.inputValue, '');
  for (const bad of [[], [{ value: 'a', label: 'A' }, { value: 'a', label: 'B' }], [{ value: '', label: 'x' }]])
    assert.throws(() => describe(() => h('select', { options: bad, accessibilityLabel: 'Plan' }), {}), /UI_SELECT/);
});

test('a textarea takes rows and maxRows as an autosizing range', () => {
  const { props } = rendered(() => h('textarea', { accessibilityLabel: 'Note', rows: 2, maxRows: 6 }));
  assert.deepEqual([props.rows, props.maxRows], [2, 6]);
  for (const bad of [{ rows: 0 }, { rows: 2.5 }, { maxRows: 3 }, { rows: 4, maxRows: 2 }])
    assert.throws(() => describe(() => h('textarea', { accessibilityLabel: 'Note', ...bad }), {}), /UI_INPUT: rows/);
});

test('a View reading env.sizeClass or env.canDock is marked as reading the host context; runtime lowering is not', () => {
  const env = { theme: 'light', viewport: { width: 400, height: 800 }, sizeClass: 'compact', canDock: false };
  const quiet = () => h('column', { style: { padding: 4 } }, h('text', {}, 'A'));
  prepare(quiet, createView({}, env));
  assert.equal(readsHostContext(), false);
  discard();
  prepare(view => h('text', {}, view.env.sizeClass), createView({}, env));
  assert.equal(readsHostContext(), true);
  discard();
  prepare(quiet, createView({}, env));
  assert.equal(readsHostContext(), false, 'each view starts unread');
  discard();
});
