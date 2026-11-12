import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// The gallery tells apps that Liuli takes the page's typeface. That holds only while no component
// or token names a family: a family class, a font-family style, or a --font-* token would pin one.
const core = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const files = (dir) => readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
  const path = join(dir, entry.name);
  if (entry.isDirectory()) return files(path);
  return /\.tsx?$/.test(entry.name) ? [path] : [];
});
const family = /\bfont-(sans|serif|mono)\b|\bfont-\[|font-family|fontFamily|--font-[a-z]/;

test('no Liuli component or token names a font family', () => {
  const named = [...files(join(core, 'src')), join(core, 'tokens.css')].flatMap(path =>
    readFileSync(path, 'utf8').split('\n').flatMap((line, index) => family.test(line) ? [`${relative(core, path)}:${index + 1}: ${line.trim()}`] : []));
  assert.deepEqual(named, []);
});
