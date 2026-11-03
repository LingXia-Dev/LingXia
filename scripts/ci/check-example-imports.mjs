// Each example stands alone: no source file imports from another example.
// Shared specs and helpers live in packages and are imported by package name.
//   node scripts/ci/check-example-imports.mjs
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, join, normalize, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = join(dirname(fileURLToPath(import.meta.url)), '../..');
const files = execFileSync('git', ['ls-files', '-z', 'examples'], { cwd: repo, encoding: 'utf8' })
  .split('\0').filter((file) => /\.(m?[jt]sx?|cjs)$/.test(file) && !file.includes('/node_modules/'));
const specifier = /(?:\bimport\s*(?:[^'"()]*?\bfrom\s*)?|\bexport\s+[^'"]*?\bfrom\s*|\bimport\s*\(\s*|\brequire\s*\(\s*)['"](\.{1,2}\/[^'"]+)['"]/g;

const example = (path) => relative('examples', path).split(sep)[0];
const problems = [];
for (const file of files) {
  const own = example(file);
  const text = readFileSync(join(repo, file), 'utf8');
  for (const match of text.matchAll(specifier)) {
    const target = normalize(join(dirname(file), match[1]));
    if (target.startsWith(`examples${sep}`) && example(target) !== own) {
      const line = text.slice(0, match.index).split('\n').length;
      problems.push(`${file}:${line}: imports ${match[1]} from examples/${example(target)}`);
    }
  }
}
if (problems.length) {
  console.error(`Examples must not import from each other; move shared code into a package:\n  ${problems.join('\n  ')}`);
  process.exit(1);
}
console.log(`example imports: ${files.length} files, none cross examples`);
