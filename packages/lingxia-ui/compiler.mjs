import { acceptsProp } from './src/authoring/props.js';
import { build } from 'esbuild';
import ts from 'typescript';
import { collectClassUsage } from './compiler/class-usage.mjs';
import { sourceChecker, moduleChecker, parseSource } from './compiler/source.mjs';
import { moduleBanner } from './compiler/compatibility.mjs';
import { compileUtilities } from './tailwind.mjs';
import { loadStylesheet, validateTokens } from './stylesheets.mjs';
import { readFile, mkdir, writeFile, copyFile } from 'node:fs/promises';
import { resolve, dirname, join, relative, isAbsolute } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tags, fields } from './src/schema.js';

const runtime = fileURLToPath(new URL('./src/runtime.js', import.meta.url));
const has = (object, key) => Object.prototype.hasOwnProperty.call(object, key);

export function audit(source, filename) {
  const file = parseSource(filename, source);
  const checker = sourceChecker(file);
  const fail = (node, message) => {
    const { line, character } = file.getLineAndCharacterOfPosition(node.getStart(file));
    throw Error(`${filename}:${line + 1}:${character + 1}: ${message}`);
  };
  const visit = node => {
    if (ts.isJsxFragment(node)) fail(node, 'UI_FRAGMENT: return one layout node; fragments are not supported');
    if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
      const name = node.tagName.getText(file);
      if (ts.isIdentifier(node.tagName) && /^[a-z]/.test(name)) {
        if ((!has(tags, name) && name !== 'view') || name === 'radio-option') fail(node, `UI_TAG: unsupported tag <${name}>`);
        for (const prop of node.attributes.properties) {
          if (ts.isJsxSpreadAttribute(prop)) {
            for (const symbol of checker.getTypeAtLocation(prop.expression).getProperties()) {
              const key = symbol.getName();
              if (!acceptsProp(name, key)) fail(prop, `UI_PROP: unsupported ${key} on <${name}> (spread)`);
            }
          }
          if (!ts.isJsxAttribute(prop)) continue;
          const key = prop.name.getText(file);
          if (name === 'search' && ['secure','keyboard'].includes(key)) fail(prop, `UI_PROP: search owns ${key}`);
          if (!acceptsProp(name, key)) fail(prop, `UI_PROP: unsupported ${key}`);
          const style = prop.initializer && ts.isJsxExpression(prop.initializer) && prop.initializer.expression;
          if (key === 'style' && style && ts.isObjectLiteralExpression(style)) {
            for (const field of style.properties) {
              if (ts.isPropertyAssignment(field)) {
                const name = field.name.getText(file).replace(/['"]/g, '');
                if (name === 'animation') fail(field, 'UI_ANIMATION: native presentation scheduling is not connected');
                if ((!has(fields, name) && name !== 'offset') || name === 'offsetX' || name === 'offsetY')
                  fail(field, `UI_STYLE: unsupported ${name}`);
              }
            }
          }
        }
      }
    }
    if (ts.isIdentifier(node) && ['window', 'document', 'HTMLElement', 'localStorage'].includes(node.text)) {
      const parent = node.parent;
      const propertyName = parent.name === node && !ts.isShorthandPropertyAssignment(parent);
      const symbol = ts.isShorthandPropertyAssignment(parent)
        ? checker.getShorthandAssignmentValueSymbol(parent) : checker.getSymbolAtLocation(node);
      if (!propertyName && !symbol) fail(node, `UI_DOM: ${node.text} is unavailable in GPU Views`);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
}

export async function buildPages(root, pages, output, catalogs = {}) {
  const { version } = JSON.parse(await readFile(new URL('./package.json', import.meta.url), 'utf8'));
  root = resolve(root); output = resolve(output);
  for (const page of pages) {
    const source = resolve(root, page);
    const rel = relative(root, source);
    if (rel.startsWith('..') || isAbsolute(rel) || !page.endsWith('.ui.tsx')) throw Error(`Invalid GPU page path: ${page}`);
    const base = page.slice(0, -'.ui.tsx'.length);
    const config = JSON.parse(await readFile(join(root, `${base}.json`), 'utf8'));
    if (config.renderer !== 'gpu') throw Error(`${base}.json: expected renderer: gpu`);
    const catalog = catalogs[page] ?? { __names: [], __modes: {} };
    if (!Array.isArray(catalog.__names) || catalog.__names.some(name => typeof name !== 'string' || !['call', 'stream'].includes(catalog.__modes?.[name])))
      throw Error(`${page}: invalid GPU action metadata`);
    const candidates = new Set(), themes = new Map(), sourceFiles = [];
    const bundled = await build({
      stdin: { contents: `export { render } from ${JSON.stringify(source)}; export { configureClasses, describe, prepare, prepareWindows, commit, discard, reset, dispatch, createView, configureActions, takeActionRequests, receiveActionResponse, takeActionErrors, hasLocalUpdates, requestLeave } from ${JSON.stringify(runtime)};`, resolveDir: root },
      bundle: true, write: false, format: 'esm', target: 'es2020', jsxFactory: 'h', inject: [runtime],
      alias: { '@lingxia/ui': runtime },
      plugins: [{ name: 'lingxia-ui-language', setup(builder) {
        builder.onLoad({ filter: /\.css$/ }, async ({ path }) => {
          const stylesheet = await loadStylesheet(path);
          validateTokens(stylesheet);
          themes.set(path, stylesheet.toString());
          return { contents: '', loader: 'js' };
        });
        builder.onLoad({ filter: /\.[jt]sx?$/ }, async ({ path }) => {
          if (path.toLowerCase().startsWith(dirname(runtime).toLowerCase())) return;
          const contents = await readFile(path, 'utf8');
          audit(contents, path);
          const file = parseSource(path, contents);
          sourceFiles.push(file);
          const collect = node => {
            if (ts.isStringLiteralLike(node) || ts.isTemplateHead(node) || ts.isTemplateMiddle(node) || ts.isTemplateTail(node))
              for (const name of node.text.split(/\s+/)) if (name) candidates.add(name);
            ts.forEachChild(node, collect);
          };
          collect(file);
          return { contents, loader: path.endsWith('.tsx') ? 'tsx' : path.endsWith('.ts') ? 'ts' : path.endsWith('.jsx') ? 'jsx' : 'js', resolveDir: dirname(path) };
        });
      } }],
      banner: { js: moduleBanner(version) },
      footer: { js: `Object.assign(globalThis, {__lingxiaRender:render, __lingxiaDescribe:describe, __lingxiaPrepare:prepare, __lingxiaPrepareWindows:prepareWindows, __lingxiaCommit:commit, __lingxiaDiscard:discard, __lingxiaReset:reset, __lingxiaDispatch:dispatch, __lingxiaCreateView:createView, __lingxiaConfigureActions:configureActions, __lingxiaTakeActionRequests:takeActionRequests, __lingxiaReceiveActionResponse:receiveActionResponse, __lingxiaTakeActionErrors:takeActionErrors, __lingxiaHasLocalUpdates:hasLocalUpdates, __lingxiaRequestLeave:requestLeave, __lingxiaActionCatalog:${JSON.stringify(catalog)}});` },
    });
    const dest = join(output, `${base}.ui.js`);
    await mkdir(dirname(dest), { recursive: true });
    if (themes.size > 1) throw Error('UI_CSS: import one shared @theme stylesheet per page');
    const classes = await compileUtilities(candidates, [...themes.values()].join('\n'));
    const checker = moduleChecker(sourceFiles);
    const classAttributes = sourceFiles.flatMap(file => collectClassUsage(file, checker));
    const byName = new Map(classes.map(rule => [rule.name, rule]));
    for (const { value, location, tag } of classAttributes) for (const name of value.split(/\s+/).filter(Boolean)) {
      const rule = byName.get(name);
      if (!rule || rule.error) throw Error(`${location}: UI_CLASS: ${name}: ${rule?.error ?? 'unknown utility'}`);
      if (has(tags, tag) && !['row', 'column'].includes(tag) &&
          (rule.style?.flexDirection !== undefined || rule.style?.display !== undefined))
        throw Error(`${location}: UI_LAYOUT: ${name} requires a flex container (<view>, <row> or <column>), not <${tag}>`);
    }
    await writeFile(dest, bundled.outputFiles[0].text + `\nconfigureClasses(${JSON.stringify(classes)});\n`);
    await copyFile(join(root, `${base}.json`), join(output, `${base}.json`));
  }
}
