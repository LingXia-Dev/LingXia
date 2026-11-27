import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createLxMock, mount, setLx } from '@lingxia/logic/testing';

// The store is Logic: it reads `lx` as it loads, so the mock goes in before the import.
const stored = new Map();
const clipboard = { text: '' };
const went = [];
const toasts = [];
setLx(
  createLxMock({
    storage: {
      get: async key => stored.get(key),
      set: async (key, value) => void stored.set(key, value),
      delete: async key => void stored.delete(key),
    },
    clipboard: {
      readText: async () => ({ status: 'ok', text: clipboard.text }),
      writeText: async text => void (clipboard.text = text),
    },
    ui: { toast: message => void toasts.push(message) },
    app: { language: { get: () => 'zh-CN', watch: () => () => {} } },
    nav: { go: target => (went.push(target), { opened: Promise.resolve() }), back: async () => {} },
  }),
);
const store = await import('../shared/store.ts');
const { FIELD_NOTES_MD } = await import('../shared/fieldNotes.ts');
const { default: homePage } = await import('../pages/home/index.ts');
const { default: docPage } = await import('../pages/doc/index.ts');

const HUMAN = { kind: 'human', id: 'me' };
const wait = ms => new Promise(resolve => setTimeout(resolve, ms));

test('the Markdown document: a library row with its format, saved automatically as the file it was', async () => {
  const rows = await new Promise(resolve => {
    const off = store.watchLibrary(() => {
      off();
      resolve(store.libraryRows());
    });
  });
  const notes = rows.find(row => row.key === store.NOTES_KEY);
  assert.equal(notes.format, 'markdown');
  assert.equal(notes.autoSave, true);
  assert.equal(rows.find(row => row.key === 'weekly-notes').format, 'json');

  const source = await store.openSource(store.NOTES_KEY);
  assert.equal(source.format, 'markdown');
  const last = source.getState().doc.blocks.at(-1);
  source.apply({ batchId: 'e', origin: HUMAN, base: source.getState().revision, ops: [{ op: 'edit', id: last.id, find: '下次', replace: '下回' }] });
  await wait(1_700);
  // Soft wraps, the table's alignment row and the front matter are as written.
  assert.equal(store.getSavedText(store.NOTES_KEY), FIELD_NOTES_MD.replace('下次试试', '下回试试'));
  assert.equal(source.getState().dirty, false);
});

test('源文件: the file as saving writes it, a JSON one also as Markdown with what it cannot keep', async () => {
  const md = await store.sourceFile(store.NOTES_KEY);
  assert.equal(md.format, 'markdown');
  assert.equal(md.text, store.getSavedText(store.NOTES_KEY));

  const json = await store.sourceFile(store.LESSON_KEY);
  assert.equal(json.format, 'json');
  assert.equal(JSON.parse(json.text).title, (await store.openSource(store.LESSON_KEY)).getState().doc.title);
  assert.match(json.markdown, /^# /);
  assert.ok(json.lost.length > 0, 'the lesson has blocks Markdown cannot hold');
  assert.ok(json.lost.every(word => !/[A-Za-z]{4}/.test(word)), `losses are words in the app's language: ${json.lost.join('、')}`);
  assert.equal(json.doc, store.LESSON_KEY);

  const page = await mount(docPage, { params: { doc: 'photo-handbook' } });
  await page.actions.showFile();
  assert.ok(page.data.file.more > 0, 'a long file is cut for the page');
  await page.actions.copyFile({ as: 'file' });
  assert.equal(clipboard.text, (await store.sourceFile('photo-handbook')).text, 'copy takes the whole file');
  page.actions.closeFile();
  assert.equal(page.data.file, null);
  await page.unmount();
});

test('从剪贴板导入: a new Markdown document opens in the editor, its warnings shown once', async () => {
  const home = await mount(homePage);
  clipboard.text = '   ';
  await home.actions.importClipboard();
  assert.equal(toasts.at(-1), '剪贴板里没有文字');

  clipboard.text = '# 借来的笔记\n\n一段 <u>带标签</u> 的文字。';
  await home.actions.importClipboard();
  const target = went.at(-1);
  assert.equal(target.page, 'doc');
  assert.equal(target.params.mode, 'edit');
  const key = target.params.doc;
  const row = store.libraryRows()[0];
  assert.equal(row.key, key, 'an import comes first in the library');
  assert.equal(row.title, '借来的笔记');
  assert.equal(row.format, 'markdown');
  await home.unmount();

  const first = await mount(docPage, { params: { doc: key, mode: 'edit' } });
  assert.equal(first.data.format, 'markdown');
  assert.deepEqual(first.data.importNotes, ['HTML 保留为文字']);
  first.actions.dismissNotes();
  assert.deepEqual(first.data.importNotes, []);
  await first.unmount();
  const again = await mount(docPage, { params: { doc: key } });
  assert.deepEqual(again.data.importNotes, [], 'once');
  await again.unmount();
});
