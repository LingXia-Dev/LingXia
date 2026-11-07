#!/usr/bin/env node
// Builds a bench page from crates/lingxia-view/tests/bench and times it on the host through
// Logic ticks (QuickJS, release build), so View-side changes have a number to beat.
//   node corpus/bench.mjs [page=data] [ticks=200] [state-json] [root=crates/lingxia-view/tests/bench]
import { join, resolve } from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { buildPages } from '../compiler/index.mjs';

const repo = resolve(fileURLToPath(new URL('../../..', import.meta.url)));
const [name = 'data', ticks = '200', state = '{}', rootArg = 'crates/lingxia-view/tests/bench'] = process.argv.slice(2);
const root = resolve(repo, rootArg);
const dist = join(repo, 'target/corpus/bench');
await buildPages(root, [`${name}.tsx`], dist);
execFileSync('cargo', ['run', '-q', '--release', '-p', 'lingxia-view', '--example', 'bench', '--', join(dist, `${name}.js`), ticks, state],
  { cwd: repo, stdio: 'inherit' });
