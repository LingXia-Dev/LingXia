import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { render } from '@lingxia/view/testing';

const gallery = dirname(fileURLToPath(import.meta.url));
const pages = JSON.parse(readFileSync(resolve(gallery, 'lxapp.json'), 'utf8')).pages.map(page => page.name);

const forms = {
  phone: { env: { sizeClass: 'compact', viewport: { width: 393, height: 852 } } },
  desk: { env: { sizeClass: 'regular', viewport: { width: 1280, height: 800 } } },
};

const defaultData = {
  rows: [
    { key: 'light-lesson', title: '光线入门', kind: '课程', format: 'json', autoSave: false, blocks: 30, dirty: true, suggestions: 1, saveWaiting: false, agentChanged: true, readRatio: 0.4 },
    { key: 'field-notes', title: '外拍手记', kind: '笔记', format: 'markdown', autoSave: true, blocks: 16, dirty: true, suggestions: 0, saveWaiting: false, agentChanged: false },
    { key: 'weekly-notes', title: '周会纪要', kind: '会议纪要', format: 'json', autoSave: false, blocks: 9, dirty: false, suggestions: 0, saveWaiting: false, agentChanged: false },
  ],
  sections: { 'light-lesson': ['一、光从哪里来', '二、色温与曝光'] },
  docKey: 'light-lesson',
  mode: 'read',
  isCourse: true,
  dirty: false,
  saveStatus: 'saved',
  revision: 1,
  outline: [],
  savedDoc: null,
  draftDoc: null,
  waiting: 1,
  runs: [
    {
      id: 'r1',
      scenario: 'direct',
      doc: 'light-lesson',
      status: 'done',
      calls: [{ name: 'jinshu_edit', args: '{"doc":"light-lesson"}', status: 'ok', result: '{"revision":2}' }],
      outcome: '摘要已写进草稿',
    },
  ],
};

const noopActions = {
  open: async () => {},
  select: async () => {},
  syncDoc: _params => {},
  run: () => {},
  pick: () => {},
  openPage: async () => {},
};

const mockChannels = {
  async open() {
    return {
      id: 'ch_test',
      send() {},
      close() {},
      on() { return this; },
      [Symbol.asyncIterator]: async function* () {},
    };
  },
};

for (const page of pages) {
  for (const [form, input] of Object.entries(forms)) {
    test(`${page} (${form}): compiles and renders cleanly`, async () => {
      const screen = await render(
        resolve(gallery, `pages/${page}/index.tsx`),
        { ...input, data: defaultData, actions: {}, channels: mockChannels },
        { root: gallery },
      );
      assert.ok(screen.root, `${page} (${form}) rendered a root node`);
    });
  }
}

// Jinshu has no AI surface: the page shows the document; the agent drill opens only on request.
for (const [form, input] of Object.entries(forms)) {
  test(`doc (${form}): the document has the page; the agent drill opens on request`, async () => {
    const screen = await render(
      resolve(gallery, 'pages/doc/index.tsx'),
      { ...input, data: defaultData, actions: {}, channels: mockChannels },
      { root: gallery },
    );
    const texts = screen.nodes.map(node => node.text).filter(Boolean);
    assert.ok(!texts.some(text => text.includes('AI 助手')), 'no AI menu');
    // A phone keeps the bar one row: the actions sit behind More.
    if (form === 'phone') screen.press(screen.getByTestId('doc-more'));
    assert.ok(screen.getByTestId('assistant-open'));
    assert.equal(screen.queryAll(node => node.props?.['data-testid'] === 'drill-direct' || node.props?.testID === 'drill-direct').length, 0);
    assert.ok(screen.getByTestId('waiting-banner'));
    // Phone: the latest run reports under the document instead of a panel over it.
    if (form === 'phone') assert.ok(screen.getByTestId('run-strip'));
    else assert.equal(screen.queryAll(node => node.props?.['data-testid'] === 'run-strip' || node.props?.testID === 'run-strip').length, 0);
  });
}

test('home (phone): every document is a row with what waits in it', async () => {
  const screen = await render(resolve(gallery, 'pages/home/index.tsx'), { ...forms.phone, data: defaultData, actions: {}, channels: mockChannels }, { root: gallery });
  assert.ok(screen.getByTestId('doc-light-lesson'));
  assert.ok(screen.getByText('助手改过'));
  assert.ok(screen.getByText('课程 · 30 块 · JSON · 已读 40%'));
});

const byTestId = (screen, id) => {
  try {
    return [screen.getByTestId(id)];
  } catch {
    return [];
  }
};
const hasText = (screen, text) => screen.nodes.some(node => node.text === text);

test('home: each row says how it is stored; the Markdown one says it saves itself; import from the clipboard', async () => {
  let imported = 0;
  const screen = await render(
    resolve(gallery, 'pages/home/index.tsx'),
    { ...forms.phone, data: defaultData, actions: { importClipboard: async () => void imported++ }, channels: mockChannels },
    { root: gallery },
  );
  assert.equal(screen.nodes.filter(node => (node.text ?? '').includes(' · JSON')).length, 2);
  assert.ok(screen.nodes.some(node => / · Markdown · 自动保存$/.test(node.text ?? '')));
  assert.ok(!hasText(screen, '未保存'), 'the dirty Markdown row is not flagged unsaved');
  screen.press(screen.getByTestId('import-clipboard'));
  await new Promise(r => setTimeout(r, 0));
  assert.equal(imported, 1);
});

for (const [form, input] of Object.entries(forms)) {
  test(`doc (${form}): 源文件 shows the file with copy; a JSON document also exports as Markdown`, async () => {
    const copied = [];
    let asked = 0;
    const file = { doc: 'light-lesson', format: 'json', text: '{\n  "version": 1\n}\n', more: 0, markdown: '# 光线入门\n', markdownMore: 0, lost: ['视频', '应用自定义块'] };
    const actions = { showFile: async () => void asked++, copyFile: async ({ as }) => void copied.push(as), closeFile: () => {} };
    const screen = await render(resolve(gallery, 'pages/doc/index.tsx'), { ...input, data: { ...defaultData, file: null }, actions, channels: mockChannels }, { root: gallery });
    if (form === 'phone') screen.press(screen.getByTestId('doc-more'));
    screen.press(screen.getByTestId('doc-file'));
    await new Promise(r => setTimeout(r, 0));
    assert.equal(asked, 1);
    screen.rerender({ data: { file } });
    assert.ok(screen.getByTestId('file-dialog'));
    assert.ok(screen.getByText('导出为 Markdown'));
    assert.ok(screen.getByText('保存时写入的 JSON。'));
    screen.press(screen.getByTestId('file-copy'));
    await new Promise(r => setTimeout(r, 0));
    screen.press(screen.getByTestId('file-shown-markdown'));
    assert.ok(screen.getByTestId('export-lost'));
    assert.ok(screen.getByText('视频、应用自定义块'), 'losses are words, not agent messages');
    assert.ok(screen.getByText('把这篇导出为 Markdown 的样子；文档仍以 JSON 保存。'), 'the subtitle follows the tab');
    assert.equal(screen.queryAll(node => node.text === '保存时写入的 JSON。').length, 0);
    screen.press(screen.getByTestId('file-copy'));
    await new Promise(r => setTimeout(r, 0));
    assert.deepEqual(copied, ['file', 'markdown']);
  });
}

test('doc: a Markdown file has no export; a long one says how much is left out; import warnings show once', async () => {
  let dismissed = 0;
  const file = { doc: 'light-lesson', format: 'markdown', text: '# 外拍手记\n', more: 120 };
  const screen = await render(
    resolve(gallery, 'pages/doc/index.tsx'),
    { ...forms.desk, data: { ...defaultData, format: 'markdown', file, importNotes: ['HTML 保留为文字', '五、六级标题改为四级'] }, actions: { dismissNotes: () => void dismissed++ }, channels: mockChannels },
    { root: gallery },
  );
  assert.equal(screen.queryAll(node => node.text === '导出为 Markdown').length, 0);
  assert.ok(screen.getByText('还有 120 行未显示，复制可得全文。'));
  assert.ok(screen.getByTestId('import-notes'));
  assert.ok(screen.getByText('导入时有些内容无法原样保留'));
  assert.ok(screen.getByText('HTML 保留为文字、五、六级标题改为四级'));
});

test('doc: one document\'s file never shows over another; the dialog belongs to its document', async () => {
  const lesson = { doc: 'light-lesson', format: 'json', text: '{}\n', more: 0, markdown: '# 光线入门\n', markdownMore: 0, lost: [] };
  const screen = await render(
    resolve(gallery, 'pages/doc/index.tsx'),
    { ...forms.desk, data: { ...defaultData, file: lesson }, actions: { closeFile: () => {} }, channels: mockChannels },
    { root: gallery },
  );
  screen.press(screen.getByTestId('file-shown-markdown'));
  assert.ok(byTestId(screen, 'file-dialog').length > 0);
  // The page is reused for another document while the old file is still in its data.
  screen.rerender({ data: { docKey: 'field-notes', format: 'markdown' } });
  assert.equal(byTestId(screen, 'file-dialog').length, 0, 'the lesson\'s file is not shown over field-notes');
  screen.rerender({ data: { docKey: 'light-lesson', format: 'json', file: { ...lesson } } });
  assert.ok(screen.getByText('保存时写入的 JSON。'), 'back on the lesson, the dialog starts on its own file');
});
