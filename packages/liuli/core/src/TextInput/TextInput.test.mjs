import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample, renderPage } from '../testing/harness.mjs';

const status = screen => screen.queryAll(node => node.props.role === 'status')[0];

test('Bound: named by its label, typing reaches the bound cell, and validation shows in the status region', async () => {
  const screen = await renderExample('TextInput/TextInput', 'Bound');
  const email = () => screen.getByTestId('email');
  assert.equal(email().props.accessibilityLabel, 'Email');
  assert.equal(email().props.placeholder, 'you@example.com');
  assert.equal(email().props.stateRequired, 1);
  assert.deepEqual(described(screen, email()), ['We send the receipt here.']);
  assert.equal(textOf(status(screen)), '');
  screen.type(email(), 'ada');
  assert.equal(email().props.inputValue, 'ada');
  assert.equal(email().props.stateInvalid, 1);
  assert.deepEqual(described(screen, email()), ['We send the receipt here.', 'Add the part after @, such as example.com.']);
  screen.type(email(), 'ada@example.com');
  assert.equal(email().props.stateInvalid, undefined);
  assert.equal(textOf(status(screen)), '');
});

test('Search: the clear button appears with text, is named by clearLabel, and empties the field', async () => {
  const screen = await renderExample('TextInput/TextInput', 'Search');
  const clear = () => screen.queryAll(node => node.tag === 'button' && node.props.accessibilityLabel === 'Clear search');
  assert.equal(clear().length, 0);
  assert.ok(!screen.queryAll(node => node.props.accessibilityHidden === 1 && node.tag === 'text').length, 'no visible label is drawn');
  screen.type(screen.getByTestId('search'), 'groceries');
  assert.equal(clear().length, 1);
  screen.press(clear()[0]);
  assert.equal(screen.getByTestId('search').props.inputValue, '');
  assert.equal(clear().length, 0);
});

test('Disabled: the editor takes no input and the reason replaces the help text', async () => {
  const screen = await renderExample('TextInput/TextInput', 'Disabled');
  const code = screen.getByTestId('code');
  assert.equal(code.props.onInput, undefined);
  assert.equal(code.props.inputValue, 'LX-2041');
  assert.deepEqual(described(screen, code), ['Codes are fixed once sent.']);
});

test('the editor has no inset of its own and matches the Button label size', async () => {
  const screen = await renderExample('TextInput/TextInput', 'Bound');
  const editor = screen.getByTestId('email');
  assert.deepEqual([editor.props.paddingLeft, editor.props.paddingRight], [0, 0], 'the box\'s px-3 is the only inset');
  assert.equal(editor.props.fontSize, 15);
});

test('a placeholder or an unfocused value too long for the box ends in an ellipsis', async () => {
  const editor = (await renderExample('TextInput/TextInput', 'Bound')).getByTestId('email');
  assert.equal(editor.props.textOverflow, 'ellipsis');
  assert.equal(editor.props.whiteSpace, 'nowrap');
});

test('Search: one muted 16 px magnifier and a plain editor, never the platform search field', async () => {
  const screen = await renderExample('TextInput/TextInput', 'Search');
  const editor = screen.getByTestId('search');
  assert.equal(editor.tag, 'input', 'the search wire tag draws a second magnifier');
  assert.equal(editor.props.enterKeyHint, 'search');
  const box = editor.parent;
  const icons = box.children.filter(node => node.props.accessibilityHidden && node !== editor);
  assert.equal(icons.length, 1);
  assert.equal(icons[0].props.color, 'text-muted');
  assert.deepEqual([icons[0].props.width, icons[0].props.height], [16, 16]);
});

test('hasRequiredMark={false}: no asterisk is drawn, the editor is still required', async () => {
  const screen = await renderPage(`
    import { TextInput } from '@liulidesign/core/TextInput';
    export function render() {
      return <page><TextInput value="" onChange={() => {}} label="Email" isRequired hasRequiredMark={false} data-testid="email" /></page>;
    }
  `);
  assert.equal(screen.queryAll(node => node.text?.includes('*')).length, 0);
  assert.equal(screen.getByTestId('email').props.stateRequired, 1);
});

test('Adornments: fixed text sits inside the box around the editor and is read before the help text; an optional field says so', async () => {
  const screen = await renderExample('TextInput/TextInput', 'Adornments');
  const price = screen.getByTestId('price');
  const box = price.parent;
  assert.deepEqual(box.children.map(child => child.tag === 'input' ? 'input' : textOf(child)), ['$', 'input', 'USD']);
  assert.deepEqual(box.children.filter(child => child.tag === 'text').map(child => [child.props.color, child.props.shrink, child.props.fontVariantNumeric]), Array(2).fill(['text-muted', 0, 'tabular-nums']));
  assert.equal(price.props.accessibilityLabel, 'Price', 'the name stays the label alone');
  assert.deepEqual(described(screen, price), ['$', 'USD']);
  assert.deepEqual(described(screen, screen.getByTestId('site')), ['https://']);
  const weight = screen.getByTestId('weight');
  assert.deepEqual(described(screen, weight), ['kg', 'optional'], 'the unit, then that it may be left empty');
  assert.equal(weight.props.stateRequired, undefined);
  const word = screen.getByText('optional');
  assert.equal(word.props.color, 'text-muted');
  assert.equal(textOf(word.parent.children[0]), 'Weight');
});

test('Checking: busy with a spinner at the end, and still editable', async () => {
  const input = (await renderExample('TextInput/TextInput', 'Checking')).getByTestId('handle');
  assert.deepEqual([input.props.stateBusy, input.props.enabled, input.parent.children.at(-1).tag], [1, undefined, 'vector']);
});

test('ReadOnly: read-only on a sunken box, with no clear button', async () => {
  const input = (await renderExample('TextInput/TextInput', 'ReadOnly')).getByTestId('iban');
  assert.deepEqual([input.props.readOnly, input.parent.props.background, input.parent.children.length], [1, 'surface-sunken', 1]);
});

test('SignIn: the password hides its text until its button shows it, and the button names what it will do', async () => {
  const screen = await renderExample('TextInput/TextInput', 'SignIn');
  const reveal = () => screen.getByTestId('password-reveal');
  assert.equal(screen.getByTestId('password').props.secure, 1);
  assert.equal(reveal().props.accessibilityLabel, 'Show password');
  screen.press(reveal());
  assert.notEqual(screen.getByTestId('password').props.secure, 1);
  assert.equal(reveal().props.accessibilityLabel, 'Hide password');
  screen.press(reveal());
  assert.equal(screen.getByTestId('password').props.secure, 1);
  assert.equal(screen.queryAll(node => node.testId === 'sign-in-email-reveal').length, 0, 'only a password field has one');
});

test('SignIn: onBlur reaches the app, which checks the email once the person leaves it', async () => {
  const screen = await renderExample('TextInput/TextInput', 'SignIn');
  screen.type(screen.getByTestId('sign-in-email'), 'ana');
  assert.equal(screen.queryAll(node => node.text === 'Add the part after @, such as example.com.').length, 0);
  screen.blur(screen.getByTestId('sign-in-email'));
  assert.equal(screen.queryAll(node => node.text === 'Add the part after @, such as example.com.').length, 1);
});

test('ShareLink: endContent sits inside the box after the value', async () => {
  const screen = await renderExample('TextInput/TextInput', 'ShareLink');
  const input = screen.getByTestId('share-link');
  const box = input.parent;
  const copy = screen.getByTestId('copy-link');
  let at = copy;
  while (at.parent !== box) at = at.parent;
  assert.ok(box.children.indexOf(at) > box.children.indexOf(input));
  screen.press(copy);
  assert.equal(screen.queryAll(node => node.text === 'Copied.').length, 1);
});

test('RightToLeft: a value takes its own direction by type, and `dir` wins', async () => {
  const screen = await renderExample('TextInput/TextInput', 'RightToLeft');
  const direction = id => screen.getByTestId(id).props.direction;
  assert.deepEqual(['rtl-address', 'rtl-address-he', 'rtl-email', 'rtl-phone', 'rtl-mac'].map(direction), ['auto', 'auto', 'ltr', 'ltr', 'ltr']);
  const code = await renderExample('TextInput/TextInput', 'Plain');
  assert.equal(code.queryAll(node => node.tag === 'input')[0].props.direction, 'auto');
});
