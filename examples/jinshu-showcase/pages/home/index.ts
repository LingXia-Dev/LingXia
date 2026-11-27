import { definePage, lx } from '@lingxia/logic';
import { extractOutline, type JinshuDocument } from '@liulidesign/jinshu/schema';
import { docKeys, importMarkdown, libraryRows, sources, watchLibrary, type DocRow } from '../../shared/store';

export default definePage({
  data: {
    rows: [] as DocRow[],
    /** Section titles of each document, for the preview beside the list. */
    sections: {} as Record<string, string[]>,
    /** Each document's opening blocks, drawn by Jinshu in the preview. */
    excerpts: {} as Record<string, JinshuDocument>,
  },
  setup(thisPage) {
    const sync = () => {
      const rows = libraryRows();
      const sections: Record<string, string[]> = {};
      const excerpts: Record<string, JinshuDocument> = {};
      for (const key of docKeys()) {
        const doc = sources.peek(key)?.getState().doc;
        if (!doc) continue;
        sections[key] = extractOutline(doc).filter(item => item.level <= 2).map(item => item.text);
        excerpts[key] = { ...doc, blocks: doc.blocks.slice(0, 6) };
      }
      thisPage.update(d => {
        d.rows = rows;
        d.sections = sections;
        d.excerpts = excerpts;
      });
    };
    const unwatch = watchLibrary(sync);
    return {
      onShow() {
        sync();
      },
      onUnload() {
        unwatch();
      },
      actions: {
        async open({ doc, mode }: { doc: string; mode?: 'read' | 'edit' }) {
          await lx.nav.go({ page: 'doc', params: mode ? { doc, mode } : { doc } }).opened;
        },
        /** A new Markdown document from the clipboard, opened in the editor. */
        async importClipboard() {
          const read = await lx.clipboard.readText();
          const text = read.status === 'ok' ? (read.text ?? '').trim() : '';
          if (!text) {
            lx.ui.toast('剪贴板里没有文字');
            return;
          }
          const { key } = await importMarkdown(`${text}\n`);
          await lx.nav.go({ page: 'doc', params: { doc: key, mode: 'edit' } }).opened;
        },
      },
    };
  },
});
