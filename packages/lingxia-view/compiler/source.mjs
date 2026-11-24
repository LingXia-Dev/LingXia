import ts from 'typescript';
import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { createRequire } from 'node:module';

// ES5's declarations only, without DOM: utility types such as `Record` must resolve, or a map written
// with `satisfies Record<…>` loses its properties and its classes go unchecked.
const libPath = join(dirname(createRequire(import.meta.url).resolve('typescript')), 'lib.es5.d.ts');
let libFile;
const lib = () => libFile ??= ts.createSourceFile(libPath, readFileSync(libPath, 'utf8'), ts.ScriptTarget.ES2020, true);
const libOptions = { lib: ['lib.es5.d.ts'], types: [], noEmit: true };

/** Script modules a page compiles from, whose string literals hold its classes. */
export const scriptModule = /\.(?:[jt]sx?|[cm][jt]s)$/;

/** The esbuild loader for a script module: `.mts`/`.cts` are TypeScript, `.mjs`/`.cjs` JavaScript. */
export function scriptLoader(filename) {
  return /\.tsx$/.test(filename) ? 'tsx' : /\.[cm]?ts$/.test(filename) ? 'ts' : /\.jsx$/.test(filename) ? 'jsx' : 'js';
}

export function parseSource(filename, source) {
  const kind = { tsx: ts.ScriptKind.TSX, ts: ts.ScriptKind.TS, jsx: ts.ScriptKind.JSX, js: ts.ScriptKind.JS }[scriptLoader(filename)];
  return ts.createSourceFile(filename, source, ts.ScriptTarget.ES2020, true, kind);
}

// Bind this module without loading ambient browser declarations or executing
// application code. Unresolved imports stay unknown and retain runtime checks.
export function sourceChecker(file) {
  const program = ts.createProgram([file.fileName], { ...libOptions, noResolve: true, allowJs: true }, {
    getSourceFile: name => name === file.fileName ? file : name === libPath ? lib() : undefined,
    getDefaultLibFileName: () => libPath, getDefaultLibLocation: () => dirname(libPath), writeFile() {}, getCurrentDirectory: () => '',
    getDirectories: () => [], fileExists: name => name === file.fileName || name === libPath,
    readFile: () => undefined, getCanonicalFileName: name => name,
    useCaseSensitiveFileNames: () => true, getNewLine: () => '\n',
  });
  return program.getTypeChecker();
}

export function moduleChecker(files, extra = {}) {
  const options = { ...extra, ...libOptions, allowJs: true, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler };
  const host = ts.createCompilerHost(options);
  const canonical = name => host.getCanonicalFileName(resolve(name));
  const cache = new Map(files.map(file => [canonical(file.fileName), file]));
  const readSource = host.getSourceFile.bind(host);
  host.getSourceFile = (name, ...args) => cache.get(canonical(name)) ?? (canonical(name) === canonical(libPath) ? lib() : readSource(name, ...args));
  return ts.createProgram(files.map(file => file.fileName), options, host).getTypeChecker();
}
