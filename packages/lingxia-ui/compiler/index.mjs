import { build } from 'esbuild';
import ts from 'typescript';
import { collectClassUsage } from './class-usage.mjs';
import { sourceChecker, moduleChecker, parseSource } from './source.mjs';
import { moduleBanner } from './compatibility.mjs';
import { compileUtilities } from './tailwind.mjs';
import { loadStylesheet, validateTokens } from './stylesheets.mjs';
import { readFile, mkdir, writeFile, copyFile, realpath } from 'node:fs/promises';
import { resolve, dirname, join, relative, isAbsolute, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { elements, style as styleTable } from '../src/generated/contract.js';

const runtime = fileURLToPath(new URL('../src/internal.js', import.meta.url));
const publicEntry = fileURLToPath(new URL('../src/index.js', import.meta.url));
const jsx = fileURLToPath(new URL('../src/runtime/jsx.js', import.meta.url));
const host = fileURLToPath(new URL('../src/host.js', import.meta.url));
const runtimeRoot = dirname(runtime).toLowerCase();
const has = (object, key) => Object.prototype.hasOwnProperty.call(object, key);
const common = new Set(['key', 'testId', 'style', 'className', 'children']);
const editors = new Set(['input', 'textarea', 'search']);
const controls = new Set(['button', 'checkbox', 'switch', 'radio']);

export function acceptsProp(tag, key) {
  const element = elements[tag];
  return common.has(key) || has(element.props, key) ||
    Boolean(element.binding && (key === 'bind' || key === 'value' || key === element.binding.event));
}

export function styleProblem(tag, name) {
  if (!has(styleTable, name) || styleTable[name].internal) return `UI_STYLE: unknown style property ${name}`;
  if (!elements[tag].style.includes(name)) return `UI_STYLE: ${name} is not supported on <${tag}>`;
}

const position = (file, node) => {
  const { line, character } = file.getLineAndCharacterOfPosition(node.getStart(file));
  return `${line + 1}:${character + 1}`;
};

/** Every authoring problem in one module, as `file:line:col: CODE: message`. */
export function audit(source, filename) {
  const file = parseSource(filename, source);
  const checker = sourceChecker(file);
  const problems = [];
  const report = (node, message) => problems.push(`${filename}:${position(file, node)}: ${message}`);
  const visit = node => {
    if (ts.isJsxFragment(node)) report(node, 'UI_FRAGMENT: return one layout node; fragments are not supported');
    if ((ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) && ts.isIdentifier(node.tagName) && /^[a-z]/.test(node.tagName.text)) {
      const tag = node.tagName.text;
      if (!has(elements, tag)) report(node, `UI_TAG: unsupported tag <${tag}>`);
      else for (const prop of node.attributes.properties) {
        if (ts.isJsxSpreadAttribute(prop)) {
          for (const symbol of checker.getTypeAtLocation(prop.expression).getProperties())
            if (!acceptsProp(tag, symbol.getName())) report(prop, `UI_PROP: ${symbol.getName()} is not a property of <${tag}> (spread)`);
          continue;
        }
        const key = prop.name.getText(file);
        if (!acceptsProp(tag, key)) report(prop, `UI_PROP: ${key} is not a property of <${tag}>`);
        const style = key === 'style' && prop.initializer && ts.isJsxExpression(prop.initializer) && prop.initializer.expression;
        if (style && ts.isObjectLiteralExpression(style)) for (const entry of style.properties) {
          if (!ts.isPropertyAssignment(entry) && !ts.isShorthandPropertyAssignment(entry)) continue;
          const name = entry.name.getText(file).replace(/['"]/g, '');
          if (name === 'animation') report(entry, 'UI_ANIMATION: native presentation scheduling is not connected');
          else { const problem = styleProblem(tag, name); if (problem) report(entry, problem); }
        }
      }
    }
    if (ts.isIdentifier(node) && ['window', 'document', 'HTMLElement', 'localStorage'].includes(node.text)) {
      const parent = node.parent;
      const propertyName = parent.name === node && !ts.isShorthandPropertyAssignment(parent);
      const symbol = ts.isShorthandPropertyAssignment(parent) ? checker.getShorthandAssignmentValueSymbol(parent) : checker.getSymbolAtLocation(node);
      if (!propertyName && !symbol) report(node, `UI_DOM: ${node.text} is unavailable in GPU Views`);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  return problems;
}

// Each JSX site carries its source position so runtime errors name the line that built the node.
export function annotate(source, filename, label) {
  const file = parseSource(filename, source);
  const inserts = [];
  const visit = node => {
    if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
      const at = (node.typeArguments ?? node.tagName).end;
      inserts.push([at, ` __loc={${JSON.stringify(`${label}:${position(file, node)}`)}}`]);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  let output = source;
  for (const [at, text] of inserts.sort((a, b) => b[0] - a[0])) output = output.slice(0, at) + text + output.slice(at);
  return output;
}

function classProblems(rule, name, tag) {
  if (!rule) return [`UI_CLASS_UNKNOWN: ${name} is not a Tailwind utility`];
  if (rule.error) return [`UI_CLASS_UNSUPPORTED: ${name}: the native renderer does not support ${rule.error.message}`];
  if (!has(elements, tag) || rule.when?.children) return [];
  const props = [...Object.keys(rule.style ?? {}), ...(rule.gradient ? ['backgroundImage'] : []),
    ...(rule.shadow ? ['boxShadow'] : []), ...(rule.backdrop ? ['backdropFilter'] : [])];
  const state = rule.when?.state;
  const problems = props.filter(prop => !elements[tag].style.includes(prop))
    .map(prop => `UI_CLASS_ELEMENT: ${name} sets ${prop}, which <${tag}> does not support`);
  if (state === 'focus' && !editors.has(tag)) problems.push(`UI_CLASS_ELEMENT: ${name} needs an editor; <${tag}> never takes focus`);
  if (state === 'active' && !controls.has(tag)) problems.push(`UI_CLASS_ELEMENT: ${name} needs a control; <${tag}> is never pressed`);
  const layout = state && state !== 'disabled' ? props.filter(prop => !styleTable[prop]?.paint) : [];
  if (layout.length) problems.push(`UI_CLASS_ELEMENT: ${name}: ${state} styles change paint only, not ${layout.join(', ')}`);
  if (state === 'disabled' && !has(elements[tag].props, 'disabled')) problems.push(`UI_CLASS_ELEMENT: ${name} needs a disableable element, not <${tag}>`);
  return problems;
}

/**
 * Compiles GPU pages. `pages` are page source paths relative to `root`; each
 * page's sibling `.json` must declare `"renderer": "gpu"`, which is the only
 * thing that makes a page native. Output is `<base>.js` beside a copy of the config.
 */
export async function buildPages(root, pages, output, catalogs = {}) {
  const { version } = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
  root = await realpath(resolve(root)); output = resolve(output);
  for (const page of pages) {
    const source = resolve(root, page);
    const rel = relative(root, source);
    const base = page.replace(/\.[jt]sx?$/, '');
    if (rel.startsWith('..') || isAbsolute(rel) || base === page) throw Error(`Invalid GPU page path: ${page}`);
    const config = JSON.parse(await readFile(join(root, `${base}.json`), 'utf8'));
    if (config.renderer !== 'gpu') throw Error(`${base}.json: expected "renderer": "gpu"`);
    const catalog = catalogs[page] ?? { __names: [], __modes: {} };
    if (!Array.isArray(catalog.__names) || catalog.__names.some(name => typeof name !== 'string' || !['call', 'stream'].includes(catalog.__modes?.[name])))
      throw Error(`${page}: invalid GPU action metadata`);
    const candidates = new Set(), themes = new Map(), sourceFiles = [], problems = [];
    let bundled;
    try {
      bundled = await build({
        stdin: { contents: `import { render } from ${JSON.stringify(source)}; import { publishHost } from ${JSON.stringify(host)}; export { configureClasses } from ${JSON.stringify(runtime)}; publishHost(render, ${JSON.stringify(catalog)});`, resolveDir: root },
        bundle: true, write: false, format: 'esm', target: 'es2020', jsxFactory: 'h', inject: [jsx], logLevel: 'silent',
        alias: { '@lingxia/ui': publicEntry },
        plugins: [{ name: 'lingxia-ui-language', setup(builder) {
          builder.onLoad({ filter: /\.css$/ }, async ({ path }) => {
            const stylesheet = await loadStylesheet(path);
            validateTokens(stylesheet);
            themes.set(path, stylesheet.toString());
            return { contents: '', loader: 'js' };
          });
          builder.onLoad({ filter: /\.[jt]sx?$/ }, async ({ path }) => {
            if (path.toLowerCase().startsWith(runtimeRoot)) return;
            const contents = await readFile(path, 'utf8');
            problems.push(...audit(contents, path));
            const file = parseSource(path, contents);
            sourceFiles.push(file);
            const collect = node => {
              if (ts.isStringLiteralLike(node) || ts.isTemplateHead(node) || ts.isTemplateMiddle(node) || ts.isTemplateTail(node))
                for (const name of node.text.split(/\s+/)) if (name) candidates.add(name);
              ts.forEachChild(node, collect);
            };
            collect(file);
            const loader = path.endsWith('.tsx') ? 'tsx' : path.endsWith('.ts') ? 'ts' : path.endsWith('.jsx') ? 'jsx' : 'js';
            const annotated = loader.endsWith('x') ? annotate(contents, path, relative(root, path).split('\\').join('/')) : contents;
            return { contents: annotated, loader, resolveDir: dirname(path) };
          });
        } }],
        banner: { js: moduleBanner(version) },
      });
    } catch (error) {
      if (!problems.length) throw error;
    }
    if (themes.size > 1) problems.push(`${page}: UI_CSS: import one shared @theme stylesheet per page`);
    const classes = await compileUtilities(candidates, [...themes.values()].join('\n'));
    const checker = moduleChecker(sourceFiles);
    const rulesByName = new Map();
    for (const rule of classes) rulesByName.set(rule.name, [...(rulesByName.get(rule.name) ?? []), rule]);
    for (const { value, location, tag } of sourceFiles.flatMap(file => collectClassUsage(file, checker)))
      for (const name of value.split(/\s+/).filter(Boolean)) {
        const found = rulesByName.get(name) ?? [undefined];
        for (const problem of new Set(found.flatMap(rule => classProblems(rule, name, tag)))) problems.push(`${location}: ${problem}`);
      }
    if (problems.length) {
      const unique = [...new Set(problems)].map(line => line.startsWith(root + sep) ? line.slice(root.length + 1) : line);
      throw Error(`${page}: ${unique.length} authoring problem${unique.length === 1 ? '' : 's'}\n${unique.join('\n')}`);
    }
    const dest = join(output, `${base}.js`);
    await mkdir(dirname(dest), { recursive: true });
    await writeFile(dest, bundled.outputFiles[0].text + `\nconfigureClasses(${JSON.stringify(classes)});\n`);
    await copyFile(join(root, `${base}.json`), join(output, `${base}.json`));
  }
}
