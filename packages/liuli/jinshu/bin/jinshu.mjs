#!/usr/bin/env node
// Runs the package's TypeScript source as it ships, on Node >= 22.15 with no build step.
import { registerHooks, stripTypeScriptTypes } from 'node:module';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// Node strips types natively, but not under node_modules nor before 22.18 without a flag; strip there ourselves.
const nativeTs = Boolean(process.features?.typescript);
const emitWarning = process.emitWarning;
process.emitWarning = (warning, ...rest) => {
  if (String(warning).includes('stripTypeScriptTypes')) return;
  emitWarning.call(process, warning, ...rest);
};

registerHooks({
  resolve(specifier, context, nextResolve) {
    // The source imports siblings without an extension, as a bundler would.
    if ((specifier.startsWith('./') || specifier.startsWith('../')) && !/\.[a-z0-9]+$/i.test(specifier) && context.parentURL) {
      const parentDir = dirname(fileURLToPath(context.parentURL));
      for (const ext of ['.ts', '/index.ts']) {
        const candidate = resolve(parentDir, specifier + ext);
        if (existsSync(candidate)) return { url: pathToFileURL(candidate).href, shortCircuit: true };
      }
    }
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    if (url.startsWith('file:') && url.endsWith('.ts') && (!nativeTs || url.includes('/node_modules/'))) {
      const source = stripTypeScriptTypes(readFileSync(fileURLToPath(url), 'utf8'));
      return { format: 'module', source, shortCircuit: true };
    }
    return nextLoad(url, context);
  },
});

const { main } = await import('./cli.mjs');
process.exitCode = await main(process.argv.slice(2));
