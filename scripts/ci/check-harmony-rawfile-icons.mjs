#!/usr/bin/env node
// Every `$rawfile('icons/…')` the Harmony SDK asks for must be an icon the
// build actually generates.
//
// These are resolved at compile time by hvigor, and the Harmony job needs a
// self-hosted runner, so it does not run on a pull request. That is how
// `icon_settings.svg` stayed referenced for months after it was moved out of
// design/icons/svg — the first sign was a release build failing with
//
//     No such 'icons/icon_settings.svg' resource in current module
//
// Comparing the two lists needs neither a runner nor a toolchain.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const ROOT = new URL('../..', import.meta.url).pathname;
const ETS_DIR = join(ROOT, 'lingxia-sdk/harmony/lingxia/src/main/ets');
const SVG_DIR = join(ROOT, 'design/icons/svg');
const REFERENCE = /\$rawfile\([^)]*?['"]icons\/([A-Za-z0-9_]+\.svg)['"]/g;

function* walk(dir) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) yield* walk(path);
    else if (path.endsWith('.ets')) yield path;
  }
}

const available = new Set(readdirSync(SVG_DIR).filter((f) => f.endsWith('.svg')));
const missing = new Map();
for (const file of walk(ETS_DIR)) {
  const source = readFileSync(file, 'utf8');
  for (const [, icon] of source.matchAll(REFERENCE)) {
    if (!available.has(icon)) {
      if (!missing.has(icon)) missing.set(icon, new Set());
      missing.get(icon).add(relative(ROOT, file));
    }
  }
}

if (missing.size > 0) {
  console.error('Harmony SDK references icons that design/icons/svg does not have:\n');
  for (const [icon, files] of [...missing].sort()) {
    console.error(`  ${icon}`);
    for (const file of [...files].sort()) console.error(`    ${file}`);
  }
  console.error('\nAdd the SVG, or point the reference at one that exists.');
  process.exit(1);
}
console.log(`Harmony rawfile icons resolve (${available.size} available)`);
