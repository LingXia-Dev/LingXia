import ts from 'typescript';
import { existsSync } from 'node:fs';
import { join } from 'node:path';

// Options from the lxapp's tsconfig that change how esbuild emits or resolves View code.
const carried = ['experimentalDecorators', 'useDefineForClassFields', 'verbatimModuleSyntax', 'preserveValueImports'];

/**
 * The one tsconfig every bundled file compiles under: the lxapp's own (`extends` resolved), with
 * JSX pinned to `h`. Without it esbuild reads the nearest tsconfig per file, so a dependency's
 * `"jsx": "react-jsx"` would switch its components to a JSX runtime the View does not have.
 */
export function viewTsconfig(root) {
  const compilerOptions = { jsx: 'react', jsxFactory: 'h', jsxFragmentFactory: 'Fragment' };
  const path = join(root, 'tsconfig.json');
  if (!existsSync(path)) return { compilerOptions };
  const { config, error } = ts.readConfigFile(path, ts.sys.readFile);
  if (error) throw Error(`${path}: ${ts.flattenDiagnosticMessageText(error.messageText, '\n')}`);
  const { options } = ts.parseJsonConfigFileContent(config, ts.sys, root, undefined, path);
  // Paths without a baseUrl resolve from the tsconfig that declared them.
  if (options.paths) compilerOptions.paths = options.paths;
  const base = options.baseUrl ?? (options.paths ? options.pathsBasePath : undefined);
  if (base) compilerOptions.baseUrl = base;
  for (const key of carried) if (options[key] !== undefined) compilerOptions[key] = options[key];
  return { compilerOptions };
}

/** The audit's type checker sees each page's generated `./$types` beside it, as the lxapp's tsconfig `rootDirs` does. */
export function checkerOptions(root) {
  return { rootDirs: [root, join(root, '.lingxia', 'types')] };
}
