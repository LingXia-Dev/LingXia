import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage } from '../testing/harness.mjs';

test('Answer: each mark reads its number and is described by its source\'s card; a pill names its source and opens it', async () => {
  const screen = await renderExample('Citation/Citation', 'Answer');
  const mark = screen.getByTestId('answer-mark-forecast');
  assert.equal(textOf(mark).trim(), '2');
  screen.press(mark);
  const card = screen.getByTestId('answer-card-forecast');
  assert.equal(textOf(card.children[0]), 'Coast forecast for Saturday');
  const pill = screen.getByTestId('answer-source-ferry');
  assert.equal(pill.props.accessibilityLabel, 'Source 1: Ferry timetable, summer, ferries.example.com');
  assert.equal(screen.getByTestId('answer-source-notes').props.accessibilityLabel, 'Source 3: Trip notes', 'no site, so the title alone');
  screen.press(pill);
  screen.getByText('Opened Ferry timetable, summer');
});

test('Answer: in Chinese, the pills are named in the page\'s language', async () => {
  const screen = await renderExample('Citation/Citation', 'Answer', { env: { language: 'zh-CN' } });
  assert.equal(screen.getByTestId('answer-source-notes').props.accessibilityLabel, '来源 3：Trip notes');
});

test('A Citation outside a group, or naming no source, is refused', async () => {
  const outside = 'import { Citation } from "@liulidesign/core/Citation";\nexport function render() { return <page><text>Claim<Citation source="a" /></text></page>; }\n';
  await assert.rejects(renderPage(() => outside), /goes inside a CitationGroup/);
});
