import ts from 'typescript';
import { parseSource } from './source.mjs';

const functionLike = node => ts.isArrowFunction(node) || ts.isFunctionExpression(node) || ts.isFunctionDeclaration(node) ||
  ts.isMethodDeclaration(node) || ts.isGetAccessor(node) || ts.isSetAccessor(node) || ts.isConstructorDeclaration(node);

function bindingNames(name, into, declaration, kind) {
  if (ts.isIdentifier(name)) into.set(name.text, { declaration, kind });
  else for (const element of name.elements ?? []) if (!ts.isOmittedExpression(element)) bindingNames(element.name, into, declaration, kind);
}

const declarationKind = list => list.flags & ts.NodeFlags.Const ? 'const' : list.flags & ts.NodeFlags.Let ? 'let' : 'var';

/** Names `scope` (a function or the module) declares itself, not inside the functions it holds. */
function ownDeclarations(scope) {
  const names = new Map();
  for (const parameter of scope.parameters ?? []) bindingNames(parameter.name, names, parameter, 'parameter');
  const visit = node => {
    if (ts.isVariableDeclarationList(node)) for (const declaration of node.declarations)
      bindingNames(declaration.name, names, declaration, declarationKind(node));
    if ((ts.isFunctionDeclaration(node) || ts.isClassDeclaration(node)) && node.name)
      names.set(node.name.text, { declaration: node, kind: ts.isFunctionDeclaration(node) ? 'function' : 'class' });
    if (ts.isCatchClause(node) && node.variableDeclaration) bindingNames(node.variableDeclaration.name, names, node.variableDeclaration, 'let');
    if (ts.isImportDeclaration(node)) {
      const clause = node.importClause;
      if (clause?.name) names.set(clause.name.text, { declaration: node, kind: 'import' });
      const bindings = clause?.namedBindings;
      if (bindings && ts.isNamespaceImport(bindings)) names.set(bindings.name.text, { declaration: node, kind: 'import' });
      if (bindings && ts.isNamedImports(bindings)) for (const element of bindings.elements) names.set(element.name.text, { declaration: node, kind: 'import' });
      return;
    }
    if (node !== scope && (functionLike(node) || ts.isClassLike(node))) return;
    ts.forEachChild(node, visit);
  };
  ts.forEachChild(scope, visit);
  return names;
}

/** Identifiers `fn` reads that it does not declare itself; `null` when it reads `this` or `arguments`. */
function freeNames(fn) {
  const own = new Set(), free = new Map();
  let impure = false;
  const collect = scope => { for (const name of ownDeclarations(scope).keys()) own.add(name); };
  collect(fn);
  const visit = node => {
    if (impure || ts.isTypeNode(node)) return;
    if (node.kind === ts.SyntaxKind.ThisKeyword || node.kind === ts.SyntaxKind.SuperKeyword) { impure = true; return; }
    if (functionLike(node)) collect(node);
    if (ts.isIdentifier(node)) {
      const parent = node.parent;
      const named = (ts.isPropertyAccessExpression(parent) && parent.name === node) ||
        (ts.isPropertyAssignment(parent) && parent.name === node) || ts.isJsxAttribute(parent) ||
        ((ts.isJsxOpeningElement(parent) || ts.isJsxSelfClosingElement(parent) || ts.isJsxClosingElement(parent)) &&
          parent.tagName === node && /^[a-z]/.test(node.text)) ||
        ((ts.isBindingElement(parent) || ts.isParameter(parent) || ts.isVariableDeclaration(parent)) && parent.name === node) ||
        (ts.isBindingElement(parent) && parent.propertyName === node) || ts.isLabeledStatement(parent) ||
        ts.isBreakOrContinueStatement(parent);
      if (node.text === 'arguments') impure = true;
      else if (!named && !own.has(node.text) && !free.has(node.text)) free.set(node.text, node);
    }
    ts.forEachChild(node, visit);
  };
  ts.forEachChild(fn, visit);
  return impure ? null : free;
}

/** Whether `name` is assigned anywhere inside `scope`. */
function reassigned(scope, name) {
  let found = false;
  const target = node => {
    while (ts.isParenthesizedExpression(node)) node = node.expression;
    if (ts.isIdentifier(node)) return node.text === name;
    if (ts.isArrayLiteralExpression(node)) return node.elements.some(element => target(ts.isSpreadElement(element) ? element.expression : element));
    if (ts.isObjectLiteralExpression(node)) return node.properties.some(property =>
      ts.isShorthandPropertyAssignment(property) ? property.name.text === name
        : ts.isPropertyAssignment(property) ? target(property.initializer) : ts.isSpreadAssignment(property) && target(property.expression));
    return false;
  };
  const visit = node => {
    if (found) return;
    if (ts.isBinaryExpression(node) && node.operatorToken.kind >= ts.SyntaxKind.FirstAssignment &&
        node.operatorToken.kind <= ts.SyntaxKind.LastAssignment && target(node.left)) found = true;
    else if ((ts.isPrefixUnaryExpression(node) || ts.isPostfixUnaryExpression(node)) &&
        (node.operator === ts.SyntaxKind.PlusPlusToken || node.operator === ts.SyntaxKind.MinusMinusToken) && target(node.operand)) found = true;
    else if ((ts.isForOfStatement(node) || ts.isForInStatement(node)) && !ts.isVariableDeclarationList(node.initializer) && target(node.initializer)) found = true;
    else ts.forEachChild(node, visit);
  };
  visit(scope);
  return found;
}

/**
 * What a closure created during render captures from its enclosing render, or `null` when its
 * identity cannot stand for its behaviour: it reads `this` or `arguments`, or a captured binding is
 * reassigned or not yet initialised where the closure is made. Module `let`/`var` bindings count as
 * captures; module constants, imports and globals do not.
 */
function captures(fn, file, moduleNames) {
  const free = freeNames(fn);
  if (!free) return null;
  const scopes = [];
  for (let owner = fn.parent; owner && !ts.isSourceFile(owner); owner = owner.parent)
    if (functionLike(owner)) scopes.push([owner, ownDeclarations(owner)]);
  const names = [];
  for (const name of free.keys()) {
    const found = scopes.find(([, declared]) => declared.has(name));
    if (!found) {
      const binding = moduleNames.get(name);
      if (binding && (binding.kind === 'let' || binding.kind === 'var')) names.push(name);
      continue;
    }
    const [scope, declared] = found, { declaration, kind } = declared.get(name);
    if (kind !== 'function' && kind !== 'parameter' && declaration.end > fn.getStart(file)) return null;
    if (kind !== 'const' && kind !== 'function' && kind !== 'class' && reassigned(scope, name)) return null;
    names.push(name);
  }
  return names;
}

// Where a closure made during render is handed on: a JSX attribute's value (or a branch of it) and a
// `const` a render function declares.
function handedOn(fn) {
  let node = fn, parent = fn.parent;
  while (ts.isParenthesizedExpression(parent) || ts.isConditionalExpression(parent) && parent.condition !== node ||
    ts.isBinaryExpression(parent) && [ts.SyntaxKind.AmpersandAmpersandToken, ts.SyntaxKind.BarBarToken,
      ts.SyntaxKind.QuestionQuestionToken].includes(parent.operatorToken.kind) || ts.isAsExpression(parent) ||
    ts.isSatisfiesExpression(parent) || ts.isPropertyAssignment(parent) && parent.initializer === node ||
    ts.isObjectLiteralExpression(parent)) {
    node = parent; parent = parent.parent;
  }
  if (ts.isJsxExpression(parent) && ts.isJsxAttribute(parent.parent)) return true;
  if (ts.isVariableDeclaration(parent) && parent.initializer === node && ts.isIdentifier(parent.name) &&
      ts.isVariableDeclarationList(parent.parent) && parent.parent.flags & ts.NodeFlags.Const) {
    for (let owner = parent.parent.parent; owner && !ts.isSourceFile(owner); owner = owner.parent) if (functionLike(owner)) return true;
  }
  return false;
}

/**
 * Wraps each closure a render hands on in `__lxFn(site, fn, captures)`: while its captures are
 * the same values, a render gets back the function an earlier render made, so props holding it
 * compare equal and the subtree built from them can stand.
 */
export function stabilizeClosures(source, filename, label) {
  if (!/=>|function/.test(source)) return source;
  const file = parseSource(filename, source);
  const moduleNames = ownDeclarations(file), edits = [];
  const visit = node => {
    if ((ts.isArrowFunction(node) || ts.isFunctionExpression(node)) && handedOn(node)) {
      const names = captures(node, file, moduleNames);
      if (names) edits.push([node.getStart(file), node.end, `${label}:${node.getStart(file)}`, names]);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  // Inner closures sit inside outer ones' text: splice from the end; where an inner and an outer
  // closure end together, the outer's close goes in first so the inner's lands before it.
  const splices = [];
  for (const [start, end, site, names] of edits) {
    splices.push({ at: start, rank: 1, order: -start, text: `__lxFn(${JSON.stringify(site)}, (` });
    splices.push({ at: end, rank: 0, order: start, text: `), [${names.join(', ')}])` });
  }
  splices.sort((a, b) => b.at - a.at || a.rank - b.rank || a.order - b.order);
  let output = source;
  for (const { at, text } of splices) output = output.slice(0, at) + text + output.slice(at);
  return output;
}

const volatileGlobals = new Set(['Date', 'performance']);

/** Whether a component reads what its props and state cannot tell: the clock, randomness or a module `let`. */
export function volatileComponent(fn, moduleNames) {
  let found = false;
  const visit = node => {
    if (found || ts.isTypeNode(node)) return;
    if (ts.isIdentifier(node) && !(ts.isPropertyAccessExpression(node.parent) && node.parent.name === node)) {
      const binding = moduleNames.get(node.text);
      if (volatileGlobals.has(node.text) && !binding) found = true;
      if (binding && (binding.kind === 'let' || binding.kind === 'var')) found = true;
    }
    if (ts.isPropertyAccessExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === 'Math' && node.name.text === 'random') found = true;
    ts.forEachChild(node, visit);
  };
  visit(fn);
  return found;
}

export { ownDeclarations };
