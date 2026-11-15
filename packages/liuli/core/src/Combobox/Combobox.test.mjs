import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample, renderPage } from '../testing/harness.mjs';

const fine = { env: { pointer: 'fine' } };
const options = screen => screen.queryAll(node => node.props.role === 'option');
const labels = screen => options(screen).map(option => textOf(option.children[0].children[0]));
const panel = (screen, id) => screen.queryAll(node => node.testId === `${id}-panel`);

test('Country: the field is a button named by its label, with the placeholder until something is chosen', async () => {
  const screen = await renderExample('Combobox/Combobox', 'Country');
  const field = screen.getByTestId('country');
  assert.equal(field.tag, 'button');
  assert.equal(field.props.accessibilityLabel, 'Country');
  assert.equal(field.props.stateExpanded, 0);
  assert.equal(textOf(field), 'Choose a country');
  assert.equal(field.children[0].props.color, 'text-muted', 'the placeholder is muted');
  assert.deepEqual(described(screen, field), ['Choose a country']);
  assert.equal(panel(screen, 'country').length, 0, 'closed, nothing of the panel is rendered');
});

test('Country, on touch: a sheet titled by the label; typing narrows the list; an option chooses and closes', async () => {
  const screen = await renderExample('Combobox/Combobox', 'Country');
  screen.press(screen.getByTestId('country'));
  assert.equal(screen.getByTestId('country').props.stateExpanded, 1);
  const [sheet] = panel(screen, 'country');
  assert.equal(sheet.props.accessibilityLabel, 'Country');
  assert.equal(screen.queryAll(node => node.props.role === 'heading').map(textOf).includes('Country'), true);
  assert.equal(options(screen).length, 29);
  const list = screen.queryAll(node => node.props.role === 'listbox')[0];
  assert.equal(list.props.accessibilityLabel, 'Country');
  const search = screen.getByTestId('country-search');
  assert.equal(search.props.accessibilityLabel, 'Search countries');
  assert.equal(search.props.autoFocus, 0, 'on touch the keyboard waits for a tap on the search field');

  screen.type(search, 'ni');
  assert.deepEqual(labels(screen), ['Nigeria', 'United Kingdom', 'United States'], 'any part of the label, whatever its case');
  screen.type(screen.getByTestId('country-search'), 'in');
  assert.deepEqual(labels(screen), ['India', 'Indonesia', 'Argentina', 'China', 'Spain', 'United Kingdom'], 'a label that starts with it comes first');
  screen.type(screen.getByTestId('country-search'), 'k');
  assert.deepEqual(labels(screen), ['Kenya', 'South Korea', 'United Kingdom', 'Türkiye'], 'then the start of a later word, then inside a word');
  screen.type(screen.getByTestId('country-search'), 'zz');
  assert.equal(options(screen).length, 0);
  const empty = screen.getByText('No country matches');
  assert.equal(empty.props.role, 'status');
  screen.type(screen.getByTestId('country-search'), 'ken');
  screen.press(screen.getByTestId('country-kenya'));
  assert.equal(panel(screen, 'country').length, 0);
  assert.equal(textOf(screen.getByTestId('country')), 'Kenya');
  assert.deepEqual(described(screen, screen.getByTestId('country')), ['Kenya'], 'the choice is read after the name');

  screen.press(screen.getByTestId('country'));
  assert.equal(options(screen).length, 29, 'what was typed is forgotten when it closes');
  const chosen = screen.getByTestId('country-kenya');
  assert.equal(chosen.props.stateSelected, 1);
  assert.equal(chosen.children.filter(child => child.tag === 'vector').length, 1, 'a tick on the chosen option');
  assert.equal(screen.getByTestId('country-norway').props.stateSelected, 0);
  screen.press(screen.getByTestId('country-close'));
  assert.equal(panel(screen, 'country').length, 0);
  assert.equal(textOf(screen.getByTestId('country')), 'Kenya', 'closing keeps the choice');
});

test('Country, with a mouse: a panel under the field with focus in the search field; Return takes the first match', async () => {
  const screen = await renderExample('Combobox/Combobox', 'Country', fine);
  screen.press(screen.getByTestId('country'));
  const [popover] = panel(screen, 'country');
  assert.deepEqual([popover.props.width, popover.props.marginTop, popover.props.accessibilityLabel], [288, 4, 'Country']);
  assert.equal(screen.queryAll(node => node.props.role === 'heading').length, 0, 'no sheet title');
  const search = screen.getByTestId('country-search');
  assert.equal(search.props.autoFocus, 1);
  screen.type(search, 'por');
  assert.deepEqual(labels(screen), ['Portugal']);
  screen.submit(screen.getByTestId('country-search'));
  assert.equal(textOf(screen.getByTestId('country')), 'Portugal');
  assert.equal(panel(screen, 'country').length, 0);
  screen.press(screen.getByTestId('country'));
  screen.escape();
  assert.equal(panel(screen, 'country').length, 0);
  assert.equal(textOf(screen.getByTestId('country')), 'Portugal');
});

test('Assignee: the help text follows the choice in the description; a second line per option; a disabled option cannot be chosen', async () => {
  const screen = await renderExample('Combobox/Combobox', 'Assignee');
  assert.deepEqual(described(screen, screen.getByTestId('assignee')), ['Ben Okafor', 'They are notified when the task changes.']);
  screen.press(screen.getByTestId('assignee'));
  const dev = screen.getByTestId('assignee-dev');
  assert.equal(textOf(dev), 'Dev PatelOn leave until June');
  assert.equal(dev.props.onClick, undefined);
  screen.type(screen.getByTestId('assignee-search'), 'dev');
  screen.submit(screen.getByTestId('assignee-search'));
  assert.equal(textOf(screen.getByTestId('assignee')), 'Ben Okafor', 'Return skips an option that cannot be chosen');
});

test('OwnMatching: with onQueryChange the list shows the options the app passes', async () => {
  const screen = await renderExample('Combobox/Combobox', 'OwnMatching');
  screen.press(screen.getByTestId('airport'));
  screen.type(screen.getByTestId('airport-search'), 'lgw');
  assert.deepEqual(options(screen).map(textOf), ['LondonLGW · Gatwick'], 'a code finds its airport, which the label alone would not');
  screen.press(screen.getByTestId('airport-LGW'));
  assert.equal(textOf(screen.getByTestId('airport')), 'London');
});

test('a long list shows fifty options and says there are more; loading shows a spinner in place of the list', async () => {
  const page = props => `
    import { Combobox } from '@liulidesign/core/Combobox';
    const options = Array.from({ length: 120 }, (_, index) => ({ value: String(index), label: 'Item ' + index }));
    export function render() { return <page><Combobox label="Item" value="" onChange={() => {}} options={options} searchLabel="Search" emptyLabel="None" data-testid="item" ${props} /></page>; }
  `;
  const screen = await renderPage(page('moreLabel="Keep typing to see more"'));
  screen.press(screen.getByTestId('item'));
  assert.equal(options(screen).length, 50);
  screen.getByText('Keep typing to see more');
  screen.type(screen.getByTestId('item-search'), 'item 11');
  assert.equal(options(screen).length, 11);
  assert.equal(screen.queryAll(node => node.text === 'Keep typing to see more').length, 0);
  const loading = await renderPage(page('isLoading'));
  loading.press(loading.getByTestId('item'));
  assert.equal(options(loading).length, 0);
  assert.equal(loading.queryAll(node => node.text === 'None').length, 0);
  assert.equal(loading.queryAll(node => node.tag === 'vector' && /"name":"spin"/.test(node.props.animation ?? '')).length, 1);
});

test('states: the sizes; an error is read after the choice and marks the edge; disabled takes no press', async () => {
  const sizes = await renderExample('Combobox/Combobox', 'Sizes');
  assert.deepEqual([sizes.getByTestId('small').props.minHeight, sizes.getByTestId('large').props.minHeight], [32, 48]);
  const invalid = await renderExample('Combobox/Combobox', 'StateInvalid');
  const field = invalid.getByTestId('invalid');
  assert.equal(field.props.borderColor, 'danger');
  assert.deepEqual(described(invalid, field), ['Choose a country', 'Choose where to ship to.']);
  const disabled = (await renderExample('Combobox/Combobox', 'StateDisabled')).getByTestId('disabled');
  assert.equal(disabled.props.onClick, undefined);
  assert.equal(textOf(disabled), 'Kenya');
});

test('Shipping: the chosen value is what the form saves', async () => {
  const screen = await renderExample('Combobox/Combobox', 'Shipping');
  screen.press(screen.getByTestId('ship-to'));
  screen.type(screen.getByTestId('ship-to-search'), 'swe');
  screen.press(screen.getByTestId('ship-to-sweden'));
  screen.press(screen.getByTestId('save-address'));
  assert.equal(textOf(screen.getByTestId('saved')), 'Saved: sweden');
});
