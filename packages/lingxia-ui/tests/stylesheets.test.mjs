import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { loadStylesheet } from '../compiler/stylesheets.mjs';
import { compileUtilities } from '../compiler/tailwind.mjs';
import { configureClasses, resolveClasses } from '../src/style/classes.js';

test('nested CSS imports retain source order and theme source locations', async () => {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-ui-theme-'));
  try {
    await writeFile(join(root, 'base.css'), '@theme { --radius-brand: 1px; }');
    await writeFile(join(root, 'tokens.css'), '@import "./base.css"; @theme { --radius-brand: 2px; }');
    await writeFile(join(root, 'theme.css'), '@import "./tokens.css"; @theme { --radius-brand: 3px; }');
    configureClasses(await compileUtilities(new Set(['rounded-brand']), (await loadStylesheet(join(root, 'theme.css'))).toString()));
    assert.equal(resolveClasses('rounded-brand').base.borderRadius, 3);
    await writeFile(join(root, 'base.css'), '@import "./theme.css";');
    await assert.rejects(loadStylesheet(join(root, 'theme.css')), /UI_CSS: cyclic import/);
    await writeFile(join(root, 'base.css'), '@import "https://example.com/theme.css";');
    await assert.rejects(loadStylesheet(join(root, 'theme.css')), /base.css.*UI_CSS/);
    await writeFile(join(root, 'base.css'), '@import "./tokens.css" screen;');
    await assert.rejects(loadStylesheet(join(root, 'theme.css')), /without conditions/);
  } finally { await rm(root, { recursive: true, force: true }); }
});
