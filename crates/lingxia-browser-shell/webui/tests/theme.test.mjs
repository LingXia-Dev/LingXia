import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

// The runtime stamps <html data-theme="light|dark"> with the product
// appearance and updates it live; system pages must theme from that stamp,
// not from the WebView's own prefers-color-scheme.
// Per file: the surfaces it paints in light, each of which the dark block must
// repaint. A missing one leaves a white panel on a dark page.
const STYLED = {
  'pages/newtab/index.html': [':root'],
  'pages/settings/index.html': [':root', '.sidebar', '.main-header', '.settings-content .section'],
  'pages/downloads/index.html': ['#clearBtn'],
  'pages/history/index.html': [':root', '.header', '.day-card', '.search'],
  'pages/bookmarks/index.html': [':root', 'body', '.rail'],
  'public/downloads.css': [':root', 'body', '.header', '.item'],
};

// Declarations of every rule whose selector list names `selector` under the
// dark stamp, joined.
function darkDeclarations(source, selector) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const target = selector === ':root'
    ? ':root\\[data-theme="dark"\\]'
    : `\\[data-theme="dark"\\] ${escaped}`;
  const rule = new RegExp(`(?:^|[,}\\s])${target}\\s*(?:,[^{]*)?\\{([^}]*)\\}`, 'g');
  return [...source.matchAll(rule)].map((match) => match[1]).join(';');
}

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), 'utf8');

for (const [path, surfaces] of Object.entries(STYLED)) {
  test(`${path} has a dark theme keyed on data-theme`, async () => {
    const source = await read(path);
    assert.doesNotMatch(source, /color-scheme:\s*light\s*;/, 'a light-only color-scheme pin defeats dark');
    for (const surface of surfaces) {
      const declarations = darkDeclarations(source, surface);
      assert.ok(declarations, `${path}: no dark rule for ${surface}`);
      if (surface === ':root') {
        assert.match(declarations, /color-scheme:\s*dark/, `${path}: dark root must set color-scheme`);
      } else {
        assert.match(declarations, /\b(background|color)\s*:/, `${path}: dark ${surface} repaints nothing`);
      }
    }
  });
}

test('pages that rely on downloads.css get its dark rules', async () => {
  for (const path of ['pages/settings/index.html', 'pages/downloads/index.html']) {
    assert.match(await read(path), /href="lingxia:\/\/lxapp\/public\/downloads\.css"/);
  }
});

test('newtab follows prefers-color-scheme only for unstamped documents', async () => {
  const source = await read('pages/newtab/index.html');
  const media = source.match(/@media \(prefers-color-scheme: dark\)\s*\{\s*([^{]+)\{/);
  assert.ok(media, 'newtab keeps a media fallback');
  assert.equal(media[1].trim(), ':root:not([data-theme])');
  const root = darkDeclarations(source, ':root');
  for (const token of ['--page', '--surface', '--text', '--muted', '--accent', '--border']) {
    assert.match(root, new RegExp(`${token}\\s*:`), `newtab dark root must define ${token}`);
  }
});
