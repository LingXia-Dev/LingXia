import { build } from 'esbuild';
import ts from 'typescript';
import { collectClassUsage } from './class-usage.mjs';
import { perfWarnings } from './perf.mjs';
import { sourceChecker, moduleChecker, parseSource } from './source.mjs';
import { moduleBanner } from './compatibility.mjs';
import { compileUtilities } from './tailwind.mjs';
import { nearestSupported } from './suggest.mjs';
import { loadStylesheet, validateTokens } from './stylesheets.mjs';
import { colorProblem, seriesProblem } from '../src/style/color.js';
import { readFile, mkdir, writeFile, copyFile, realpath } from 'node:fs/promises';
import { resolve, dirname, join, relative, isAbsolute, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { elements, style as styleTable, commonProps } from '../src/generated/contract.js';

const runtime = fileURLToPath(new URL('../src/internal.js', import.meta.url));
const publicEntry = fileURLToPath(new URL('../src/index.js', import.meta.url));
const jsx = fileURLToPath(new URL('../src/runtime/jsx.js', import.meta.url));
const host = fileURLToPath(new URL('../src/host.js', import.meta.url));
const runtimeRoot = dirname(runtime).toLowerCase();
const has = (object, key) => Object.prototype.hasOwnProperty.call(object, key);
const common = new Set(commonProps);
const editors = new Set(['input', 'textarea', 'search']);
const controls = new Set(['button', 'checkbox', 'switch', 'radio', 'link']);

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
// Controls a screen reader announces need a name; the fix is named in the message.
const labelled = new Set(['input', 'textarea', 'search', 'slider', 'radio', 'plot', 'select']);
const contentNamed = new Set(['button', 'link', 'checkbox', 'switch']);
function unnamed(tag, node, file) {
  const attributes = node.attributes.properties;
  if (attributes.some(ts.isJsxSpreadAttribute)) return undefined;
  const has = name => attributes.some(prop => prop.name?.getText(file) === name);
  if (tag === 'image' && !has('alt')) return 'UI_A11Y: <image> needs alt (describe it, or alt="" when decorative)';
  if (labelled.has(tag) && !has('accessibilityLabel'))
    return `UI_A11Y: <${tag}> needs an accessible name; add accessibilityLabel="…"${editors.has(tag) ? ' (a placeholder is a hint, not a name)' : ''}`;
  if (contentNamed.has(tag) && !has('accessibilityLabel') && ts.isJsxOpeningElement(node) && !spoken(node.parent))
    return `UI_A11Y: <${tag}> has no text to name it; add text or accessibilityLabel="…"`;
  if (contentNamed.has(tag) && !has('accessibilityLabel') && ts.isJsxSelfClosingElement(node))
    return `UI_A11Y: <${tag}> has no text to name it; add text or accessibilityLabel="…"`;
  return undefined;
}
// Whether children may produce text: literal text, an expression, or a text element.
function spoken(element) {
  return element.children.some(child => {
    if (ts.isJsxText(child)) return child.text.trim() !== '';
    if (ts.isJsxExpression(child)) return Boolean(child.expression);
    if (ts.isJsxElement(child)) {
      const name = child.openingElement.tagName.getText();
      return ['text', 'span'].includes(name) || /^[A-Z]/.test(name) || spoken(child);
    }
    if (ts.isJsxSelfClosingElement(child)) return /^[A-Z]/.test(child.tagName.getText());
    return false;
  });
}

export function audit(source, filename, { literalColors = false } = {}) {
  const file = parseSource(filename, source);
  const checker = sourceChecker(file);
  const problems = [];
  const report = (node, message) => problems.push(`${filename}:${position(file, node)}: ${message}`);
  const visit = node => {
    if (ts.isJsxFragment(node)) report(node, 'UI_FRAGMENT: return one layout node; fragments are not supported');
    if ((ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) && ts.isIdentifier(node.tagName) && /^[a-z]/.test(node.tagName.text)) {
      const tag = node.tagName.text;
      const problem = has(elements, tag) && unnamed(tag, node, file);
      if (problem) report(node, problem);
      if (!has(elements, tag)) report(node, `UI_TAG: unsupported tag <${tag}>`);
      else for (const prop of node.attributes.properties) {
        if (ts.isJsxSpreadAttribute(prop)) {
          for (const symbol of checker.getTypeAtLocation(prop.expression).getProperties())
            if (!acceptsProp(tag, symbol.getName())) report(prop, `UI_PROP: ${symbol.getName()} is not a property of <${tag}> (spread)`);
          continue;
        }
        const key = prop.name.getText(file);
        if (!acceptsProp(tag, key))
          report(prop, `UI_PROP: ${key} is not a property of <${tag}>${key === 'onClick' && acceptsProp(tag, 'onPress') ? '; use onPress' : ''}`);
        const series = tag === 'plot' && key === 'colors' && prop.initializer && ts.isJsxExpression(prop.initializer) && prop.initializer.expression;
        if (!literalColors && series && ts.isArrayLiteralExpression(series)) for (const item of series.elements) {
          const problem = ts.isStringLiteralLike(item) && seriesProblem(item.text);
          if (problem) report(item, problem);
        }
        const style = key === 'style' && prop.initializer && ts.isJsxExpression(prop.initializer) && prop.initializer.expression;
        if (style && ts.isObjectLiteralExpression(style)) for (const entry of style.properties) {
          if (!ts.isPropertyAssignment(entry) && !ts.isShorthandPropertyAssignment(entry)) continue;
          const name = entry.name.getText(file).replace(/['"]/g, '');
          if (name === 'animation') report(entry, 'UI_ANIMATION: inline animation is not supported; use animate-* utilities');
          else { const problem = styleProblem(tag, name); if (problem) report(entry, problem); }
          const literal = ts.isPropertyAssignment(entry) && ts.isStringLiteralLike(entry.initializer) && entry.initializer.text;
          const colorIssue = !literalColors && literal !== false && styleTable[name]?.lower === 'color' && colorProblem(literal, name);
          if (colorIssue) report(entry, colorIssue);
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

// Flex-only properties on a grid name the grid property that does the same job.
const hints = { grid: {
  flexWrap: '; grid items wrap into rows by themselves (use grid-cols-[repeat(auto-fit,minmax(…,1fr))])',
  flexDirection: '; use grid-flow-col to fill columns first',
} };
// Flex elements center with the flex pair; `place-items-*` and `justify-items-*` are grid-only.
const flexHints = { justifyItems: '; on a flex element center with items-center justify-center' };

function classProblems(rule, name, tag, suggestions) {
  if (!rule) {
    const nearest = suggestions.get(name);
    return [`UI_CLASS_UNKNOWN: ${name} is not a Tailwind utility${nearest ? `; nearest supported: ${nearest}` : ''}`];
  }
  if (rule.error?.category === 'color') return [`UI_COLOR: ${name}: ${rule.error.message}`];
  if (rule.error) return [`UI_CLASS_UNSUPPORTED: ${name}: the native renderer does not support ${rule.error.message}`];
  if (!has(elements, tag) || rule.when?.children) return [];
  const runs = tag === 'span' || tag === 'link';
  const props = [...Object.keys(rule.style ?? {}).filter(prop => !(runs && rule.pairedLeading && prop === 'lineHeight')), ...(rule.gradient ? ['backgroundImage'] : []),
    ...(rule.transform?.translate ? ['translate'] : []), ...(rule.transform?.scale ? ['scale'] : []),
    ...(rule.numeric ? ['fontVariantNumeric'] : []),
    ...(rule.shadow ? ['boxShadow'] : []), ...(rule.backdrop ? ['backdropFilter'] : [])];
  const state = rule.when?.state;
  const problems = props.filter(prop => !elements[tag].style.includes(prop))
    .map(prop => `UI_CLASS_ELEMENT: ${name} sets ${prop}, which <${tag}> does not support${hints[tag]?.[prop] ?? flexHints[prop] ?? ''}`);
  if (state === 'focus' && !editors.has(tag) && tag !== 'select') problems.push(`UI_CLASS_ELEMENT: ${name} needs an editor or <select>; <${tag}> never takes focus`);
  if (state === 'active' && !controls.has(tag)) problems.push(`UI_CLASS_ELEMENT: ${name} needs a control; <${tag}> is never pressed`);
  const layout = state && state !== 'disabled' ? props.filter(prop => !styleTable[prop]?.paint) : [];
  if (layout.length) problems.push(`UI_CLASS_ELEMENT: ${name}: ${state} styles change paint only, not ${layout.join(', ')}`);
  if (state === 'disabled' && !has(elements[tag].props, 'disabled')) problems.push(`UI_CLASS_ELEMENT: ${name} needs a disableable element, not <${tag}>`);
  return problems;
}

// Problems read top-down: the page first, then the files it imports, each in
// line order; one message found in several places is listed once with every place.
function report(problems, page) {
  const located = [], loose = [];
  for (const line of new Set(problems)) {
    const match = line.match(/^(.*?):(\d+):(\d+): (.*)$/s);
    if (match) located.push({ file: match[1], line: Number(match[2]), column: Number(match[3]), message: match[4] });
    else loose.push(line);
  }
  const files = [...new Set([page, ...located.map(problem => problem.file)])];
  located.sort((a, b) => files.indexOf(a.file) - files.indexOf(b.file) || a.line - b.line || a.column - b.column);
  const grouped = new Map();
  for (const problem of located) {
    const key = `${problem.file}\u0000${problem.message}`;
    if (!grouped.has(key)) grouped.set(key, { ...problem, places: [] });
    grouped.get(key).places.push(`${problem.line}:${problem.column}`);
  }
  const text = [...loose, ...[...grouped.values()].map(({ file, places, message }) => `${file}:${places.join(', ')}: ${message}`)];
  return { count: text.length, text: text.join('\n') };
}

// A warm dev compiler rebuilds the files of one page per save; unchanged ones keep their analysis.
const analyses = new Map();
function analyse(path, contents, label, loader, literalColors) {
  const key = `${literalColors}\0${label}\0${path}`;
  const cached = analyses.get(key);
  if (cached?.contents === contents) return cached;
  const file = parseSource(path, contents);
  const names = new Set();
  const collect = node => {
    if (ts.isStringLiteralLike(node) || ts.isTemplateHead(node) || ts.isTemplateMiddle(node) || ts.isTemplateTail(node))
      for (const name of node.text.split(/\s+/)) if (name) names.add(name);
    ts.forEachChild(node, collect);
  };
  collect(file);
  const analysed = {
    contents, file, names, problems: audit(contents, path, { literalColors }),
    annotated: loader.endsWith('x') ? annotate(contents, path, label) : contents,
  };
  analyses.delete(key);
  if (analyses.size >= 512) analyses.delete(analyses.keys().next().value);
  analyses.set(key, analysed);
  return analysed;
}

/**
 * Compiles GPU pages. `pages` are page source paths relative to `root`; each
 * page's sibling `.json` must declare `"renderer": "gpu"`, which is the only
 * thing that makes a page native. Output is `<base>.js` beside a copy of the config.
 * Returns the `UI_PERF` warnings, which are also printed; they never stop the build.
 */
/**
 * `literalColors` admits hex and palette colours for renderer test fixtures whose probes read exact
 * pixels; app builds never set it.
 */
export async function buildPages(root, pages, output, catalogs = {}, { dev = false, literalColors = false } = {}) {
  const { version } = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
  root = await realpath(resolve(root)); output = resolve(output);
  const inputs = {}, warnings = [];
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
        stdin: { contents: `import { render } from ${JSON.stringify(source)}; import { publishHost } from ${JSON.stringify(host)}; export { configureClasses, configureSourceMap, enableHotSwap } from ${JSON.stringify(runtime)}; publishHost(render, ${JSON.stringify(catalog)});`, resolveDir: root },
        bundle: true, write: false, format: 'esm', target: 'es2020', jsxFactory: 'h', inject: [jsx], logLevel: 'silent',
        // Dev builds keep a source map so runtime errors name the author's line.
        ...(dev ? { sourcemap: 'external', absWorkingDir: root, outfile: join(root, 'page.js'), metafile: true } : {}),
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
            const loader = path.endsWith('.tsx') ? 'tsx' : path.endsWith('.ts') ? 'ts' : path.endsWith('.jsx') ? 'jsx' : 'js';
            const label = relative(root, path).split('\\').join('/');
            const analysed = analyse(path, contents, label, loader, literalColors);
            problems.push(...analysed.problems);
            sourceFiles.push(analysed.file);
            for (const name of analysed.names) candidates.add(name);
            return { contents: analysed.annotated, loader, resolveDir: dirname(path) };
          });
        } }],
        banner: { js: moduleBanner(version) },
      });
    } catch (error) {
      if (!problems.length) throw error;
    }
    if (themes.size > 1) problems.push(`${page}: UI_CSS: import one shared @theme stylesheet per page`);
    const classes = await compileUtilities(candidates, [...themes.values()].join('\n'), { literalColors });
    const checker = moduleChecker(sourceFiles);
    const rulesByName = new Map();
    for (const rule of classes) rulesByName.set(rule.name, [...(rulesByName.get(rule.name) ?? []), rule]);
    const usages = sourceFiles.flatMap(file => collectClassUsage(file, checker));
    const unknown = [...new Set(usages.flatMap(({ value }) => value.split(/\s+/)).filter(name => name && !rulesByName.has(name)))];
    const suggestions = await nearestSupported(unknown, [...themes.values()].join('\n'));
    for (const { value, location, tag } of usages)
      for (const name of value.split(/\s+/).filter(Boolean)) {
        const found = rulesByName.get(name) ?? [undefined];
        for (const problem of new Set(found.flatMap(rule => classProblems(rule, name, tag, suggestions)))) problems.push(`${location}: ${problem}`);
      }
    if (problems.length) {
      const lines = report(problems.map(line => line.startsWith(root + sep) ? line.slice(root.length + 1) : line), rel);
      throw Error(`${page}: ${lines.count} authoring problem${lines.count === 1 ? '' : 's'}\n${lines.text}`);
    }
    const perf = await perfWarnings(root, sourceFiles, checker, rulesByName);
    if (perf.length) {
      const lines = report(perf.map(line => line.startsWith(root + sep) ? line.slice(root.length + 1) : line), rel);
      warnings.push(...lines.text.split('\n'));
      console.warn(`${page}: ${lines.count} performance warning${lines.count === 1 ? '' : 's'}\n${lines.text}`);
    }
    const dest = join(output, `${base}.js`);
    await mkdir(dirname(dest), { recursive: true });
    const code = bundled.outputFiles.find(file => !file.path.endsWith('.map'));
    const map = bundled.outputFiles.find(file => file.path.endsWith('.map'));
    let module = code.text.replace(/\n\/\/# sourceMappingURL=.*\n?$/, '\n') + `\nconfigureClasses(${JSON.stringify(classes)});\n`;
    if (map) {
      const { sources, mappings } = JSON.parse(map.text);
      // Only the author's files are named; the runtime and packages stay anonymous.
      const authored = sources.map(path => path.startsWith('..') || path.includes('node_modules') || path === '<stdin>' ? null : path.split('\\').join('/'));
      module += `configureSourceMap(${JSON.stringify({ sources: authored, mappings })});\n`;
      module += 'enableHotSwap();\n';
    }
    await writeFile(dest, module);
    // Dev builds record each page's inputs so `lingxia dev` can rebuild only the pages an edit touches.
    if (bundled.metafile) inputs[page] = Object.keys(bundled.metafile.inputs).filter(path => !path.startsWith('..') && !path.includes('node_modules') && path !== '<stdin>').map(path => path.split('\\').join('/'));
    await copyFile(join(root, `${base}.json`), join(output, `${base}.json`));
  }
  if (Object.keys(inputs).length) {
    const path = join(root, '.lingxia', 'gpu-inputs.json');
    const previous = JSON.parse(await readFile(path, 'utf8').catch(() => '{}'));
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, JSON.stringify({ ...previous, ...inputs }, null, 1) + '\n');
  }
  return { warnings };
}
