import ts from 'typescript';
import { resolve } from 'node:path';

export function parseSource(filename, source) {
  const kind = filename.endsWith('.tsx') ? ts.ScriptKind.TSX : filename.endsWith('.jsx') ? ts.ScriptKind.JSX
    : filename.endsWith('.ts') ? ts.ScriptKind.TS : ts.ScriptKind.JS;
  return ts.createSourceFile(filename, source, ts.ScriptTarget.ES2020, true, kind);
}

// Bind this module without loading ambient browser declarations or executing
// application code. Unresolved imports stay unknown and retain runtime checks.
export function sourceChecker(file) {
  const program = ts.createProgram([file.fileName], { noLib: true, noResolve: true, allowJs: true }, {
    getSourceFile: name => name === file.fileName ? file : undefined,
    getDefaultLibFileName: () => '', writeFile() {}, getCurrentDirectory: () => '',
    getDirectories: () => [], fileExists: name => name === file.fileName,
    readFile: () => undefined, getCanonicalFileName: name => name,
    useCaseSensitiveFileNames: () => true, getNewLine: () => '\n',
  });
  return program.getTypeChecker();
}

export function moduleChecker(files) {
  const options = { noLib: true, types: [], allowJs: true, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler };
  const host = ts.createCompilerHost(options);
  const canonical = name => host.getCanonicalFileName(resolve(name));
  const cache = new Map(files.map(file => [canonical(file.fileName), file]));
  const readSource = host.getSourceFile.bind(host);
  host.getSourceFile = (name, ...args) => cache.get(canonical(name)) ?? readSource(name, ...args);
  return ts.createProgram(files.map(file => file.fileName), options, host).getTypeChecker();
}
