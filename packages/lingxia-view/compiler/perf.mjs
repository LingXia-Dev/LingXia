import ts from 'typescript';
import { readFile, stat } from 'node:fs/promises';
import { resolve, relative, isAbsolute } from 'node:path';
import { stringLiterals } from './class-usage.mjs';
import { limits } from '../src/generated/contract.js';

/** Literal rows a page may build outside `<list>` before the build suggests virtualising. */
export const literalRows = 50;
/** Longest side, in pixels, of a bundled image that a phone screen can use. */
export const imageSide = 2048;

const at = node => {
  const file = node.getSourceFile();
  const { line, character } = file.getLineAndCharacterOfPosition(node.getStart(file));
  return `${file.fileName}:${line + 1}:${character + 1}`;
};
const functionLike = node => node && (ts.isArrowFunction(node) || ts.isFunctionExpression(node) || ts.isFunctionDeclaration(node));
const tagName = node => (ts.isJsxElement(node) ? node.openingElement : node).tagName;
const mapper = call => ts.isPropertyAccessExpression(call.expression) && ['map', 'flatMap'].includes(call.expression.name.text);
const arrayFrom = call => ts.isPropertyAccessExpression(call.expression) && call.expression.name.text === 'from' &&
  ts.isIdentifier(call.expression.expression) && call.expression.expression.text === 'Array';
const yieldsJsx = node => {
  let found = false;
  const visit = child => { if (found) return; if (ts.isJsxElement(child) || ts.isJsxSelfClosingElement(child)) found = true; else ts.forEachChild(child, visit); };
  visit(node);
  return found;
};

/** How many items an expression statically holds: array literals, `Array.from({ length: N })`, `Array(N)`, const bindings. */
function staticLength(checker, node, seen = new Set()) {
  if (!node || seen.has(node)) return undefined;
  seen.add(node);
  while (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression(node)) node = node.expression;
  if (ts.isArrayLiteralExpression(node)) return node.elements.some(ts.isSpreadElement) ? undefined : node.elements.length;
  if (ts.isCallExpression(node) || ts.isNewExpression(node)) {
    const [first] = node.arguments ?? [];
    if (ts.isCallExpression(node) && arrayFrom(node) && first && ts.isObjectLiteralExpression(first)) {
      const length = first.properties.find(prop => ts.isPropertyAssignment(prop) && prop.name.getText() === 'length');
      return length && ts.isNumericLiteral(length.initializer) ? Number(length.initializer.text) : undefined;
    }
    if (ts.isIdentifier(node.expression) && node.expression.text === 'Array' && first && ts.isNumericLiteral(first)) return Number(first.text);
    if (ts.isPropertyAccessExpression(node.expression) && ['fill', 'keys'].includes(node.expression.name.text))
      return staticLength(checker, node.expression.expression, seen);
  }
  if (ts.isIdentifier(node)) {
    let symbol = checker.getSymbolAtLocation(node);
    if (symbol && (symbol.flags & ts.SymbolFlags.Alias)) symbol = checker.getAliasedSymbol(symbol);
    const declaration = symbol?.valueDeclaration;
    if (declaration && ts.isVariableDeclaration(declaration) && (declaration.parent.flags & ts.NodeFlags.Const))
      return staticLength(checker, declaration.initializer, seen);
  }
  return undefined;
}

/** The function a component tag names, through imports. */
function component(checker, tag) {
  let symbol = checker.getSymbolAtLocation(tag);
  if (symbol && (symbol.flags & ts.SymbolFlags.Alias)) symbol = checker.getAliasedSymbol(symbol);
  const declaration = symbol?.valueDeclaration;
  if (functionLike(declaration)) return declaration;
  if (declaration && ts.isVariableDeclaration(declaration) && functionLike(declaration.initializer)) return declaration.initializer;
  return undefined;
}

/** Functions whose JSX is built once per row: `.map` callbacks, `<List renderItem>`, `<list row>`, and the components they render. */
function rowFunctions(files, checker) {
  const rows = new Set(), queue = [];
  const add = fn => { if (functionLike(fn) && !rows.has(fn)) { rows.add(fn); queue.push(fn); } };
  for (const file of files) {
    const visit = node => {
      if (ts.isCallExpression(node) && (mapper(node) || arrayFrom(node))) add(node.arguments[arrayFrom(node) ? 1 : 0]);
      if (ts.isJsxAttribute(node) && ['renderItem', 'row'].includes(node.name.getText()) && node.initializer && ts.isJsxExpression(node.initializer))
        add(node.initializer.expression);
      ts.forEachChild(node, visit);
    };
    visit(file);
  }
  while (queue.length) {
    const visit = node => {
      if ((ts.isJsxElement(node) || ts.isJsxSelfClosingElement(node)) && /^[A-Z]/.test(tagName(node).getText())) add(component(checker, tagName(node)));
      ts.forEachChild(node, visit);
    };
    visit(queue.pop());
  }
  return rows;
}

const composites = rule => !rule.when?.state && (
  (rule.style?.opacity ?? 1) < 1 ||
  Boolean(rule.backdrop && (rule.backdrop.blur > 0 || (rule.backdrop.saturation ?? 1) !== 1)));

/** PNG and JPEG pixel size from the file header. */
function dimensions(bytes) {
  if (bytes.length > 24 && bytes.readUInt32BE(0) === 0x89504e47) return [bytes.readUInt32BE(16), bytes.readUInt32BE(20)];
  if (bytes[0] !== 0xff || bytes[1] !== 0xd8) return undefined;
  for (let i = 2; i + 9 < bytes.length;) {
    if (bytes[i] !== 0xff) return undefined;
    const marker = bytes[i + 1];
    if (marker >= 0xc0 && marker <= 0xcf && ![0xc4, 0xc8, 0xcc].includes(marker)) return [bytes.readUInt16BE(i + 7), bytes.readUInt16BE(i + 5)];
    i += 2 + bytes.readUInt16BE(i + 2);
  }
  return undefined;
}

const mib = bytes => `${(bytes / 1048576).toFixed(1)} MiB`;

/**
 * Patterns that build but cost every frame, as `file:line:col: UI_PERF: …`: literal row sets above
 * `literalRows` outside `<list>`, opacity or backdrop layers repeated per row, and bundled images
 * too large to load or larger than any screen paints them.
 */
export async function perfWarnings(root, files, checker, rulesByName) {
  const warnings = [];
  const rows = rowFunctions(files, checker);
  const images = [];
  for (const file of files) {
    const visit = node => {
      if (ts.isCallExpression(node) && (mapper(node) || arrayFrom(node))) {
        const callback = node.arguments[arrayFrom(node) ? 1 : 0];
        const count = mapper(node) ? staticLength(checker, node.expression.expression) : staticLength(checker, node);
        if (functionLike(callback) && yieldsJsx(callback) && count > literalRows)
          warnings.push(`${at(node)}: UI_PERF: ${count} rows built at once; above ${literalRows} use <List> so only rows near the viewport exist`);
      }
      if (ts.isJsxAttribute(node) && node.initializer) {
        const name = node.name.getText();
        const expression = ts.isJsxExpression(node.initializer) ? node.initializer.expression : node.initializer;
        let owner = node.parent;
        while (owner && !functionLike(owner)) owner = owner.parent;
        const inRow = owner && rows.has(owner);
        if (inRow && name === 'className') for (const literal of stringLiterals(checker, expression))
          for (const cls of literal.text.split(/\s+/).filter(Boolean))
            if ((rulesByName.get(cls) ?? []).some(composites))
              warnings.push(`${at(literal)}: UI_PERF: ${cls} composites a layer for every row; apply it once to the container, or use a translucent colour`);
        if (inRow && name === 'style' && expression && ts.isObjectLiteralExpression(expression))
          for (const prop of expression.properties)
            if (ts.isPropertyAssignment(prop) && prop.name.getText().replace(/['"]/g, '') === 'opacity' && ts.isNumericLiteral(prop.initializer) && Number(prop.initializer.text) < 1)
              warnings.push(`${at(prop)}: UI_PERF: opacity composites a layer for every row; apply it once to the container, or use a translucent colour`);
        if (name === 'src' && tagName(node.parent.parent).getText() === 'image')
          for (const literal of stringLiterals(checker, expression)) images.push(literal);
      }
      ts.forEachChild(node, visit);
    };
    visit(file);
  }
  for (const literal of images) {
    const src = literal.text;
    if (!src || /^[a-z][a-z0-9+.-]*:/i.test(src) || src.startsWith('/')) continue;
    const path = resolve(root, src), rel = relative(root, path);
    if (rel.startsWith('..') || isAbsolute(rel)) continue;
    const size = await stat(path).then(info => info.size, () => undefined);
    if (size === undefined) continue;
    if (size > limits.imageBytes) {
      warnings.push(`${at(literal)}: UI_PERF: ${src} is ${mib(size)}; images above ${mib(limits.imageBytes)} fail to load`);
      continue;
    }
    const pixels = dimensions(await readFile(path));
    if (pixels && Math.max(...pixels) > imageSide)
      warnings.push(`${at(literal)}: UI_PERF: ${src} is ${pixels[0]}×${pixels[1]}; every load decodes it, so ship it at most ${imageSide} px on its long side`);
  }
  return warnings;
}
