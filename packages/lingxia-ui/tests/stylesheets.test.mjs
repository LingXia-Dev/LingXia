import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { loadStylesheet } from '../stylesheets.mjs';
import { compileUtilities } from '../tailwind.mjs';
import { configureClasses, resolveStyle } from '../src/classes.js';

test('nested CSS imports retain source order and theme source locations', async () => {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-ui-theme-'));
  try {
    await writeFile(join(root, 'base.css'), '@theme { --color-brand: #112233; }');
    await writeFile(join(root, 'tokens.css'), '@import "./base.css"; @theme { --color-brand: #445566; }');
    await writeFile(join(root, 'theme.css'), '@import "./tokens.css"; @theme { --color-brand: #778899; }');
    configureClasses(await compileUtilities(new Set(['bg-brand']), (await loadStylesheet(join(root, 'theme.css'))).toString()));
    assert.equal(resolveStyle('bg-brand').style.background, '#778899ff');
    await writeFile(join(root, 'base.css'), '@import "./theme.css";');
    await assert.rejects(loadStylesheet(join(root, 'theme.css')), /UI_CSS: cyclic import/);
    await writeFile(join(root, 'base.css'), '@import "https://example.com/theme.css";');
    await assert.rejects(loadStylesheet(join(root, 'theme.css')), /base.css.*UI_CSS/);
    await writeFile(join(root, 'base.css'), '@import "./tokens.css" screen;');
    await assert.rejects(loadStylesheet(join(root, 'theme.css')), /without conditions/);
  } finally { await rm(root, { recursive: true, force: true }); }
});
