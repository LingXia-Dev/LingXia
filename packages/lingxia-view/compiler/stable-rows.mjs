import ts from 'typescript';
import { parseSource } from './source.mjs';

// Globals a row may read without making its output depend on the render.
const pureGlobals = new Set(['Math', 'String', 'Number', 'Boolean', 'JSON', 'Array', 'Object', 'undefined', 'NaN',
  'Infinity', 'parseInt', 'parseFloat', 'isFinite', 'isNaN']);

function bindingNames(name, into) {
  if (ts.isIdentifier(name)) into.add(name.text);
  else for (const element of name.elements ?? []) if (!ts.isOmittedExpression(element)) bindingNames(element.name, into);
}

/** Names a function-like or module declares anywhere inside `node`. */
function declared(node, into = new Set()) {
  const visit = child => {
    if ((ts.isVariableDeclaration(child) || ts.isParameter(child) || ts.isBindingElement(child)) && child.name) bindingNames(child.name, into);
    if ((ts.isFunctionDeclaration(child) || ts.isClassDeclaration(child) || ts.isFunctionExpression(child) || ts.isClassExpression(child)) && child.name) into.add(child.name.text);
    if (ts.isCatchClause(child) && child.variableDeclaration) bindingNames(child.variableDeclaration.name, into);
    ts.forEachChild(child, visit);
  };
  visit(node);
  return into;
}

/** Module bindings that never change: imports, functions, classes and `const`s. */
function moduleConstants(file) {
  const names = new Set();
  for (const statement of file.statements) {
    if (ts.isImportDeclaration(statement) && statement.importClause && !statement.importClause.isTypeOnly) {
      const clause = statement.importClause;
      if (clause.name) names.add(clause.name.text);
      if (clause.namedBindings && ts.isNamespaceImport(clause.namedBindings)) names.add(clause.namedBindings.name.text);
      if (clause.namedBindings && ts.isNamedImports(clause.namedBindings))
        for (const element of clause.namedBindings.elements) names.add(element.name.text);
    }
    if ((ts.isFunctionDeclaration(statement) || ts.isClassDeclaration(statement)) && statement.name) names.add(statement.name.text);
    if (ts.isVariableStatement(statement) && (statement.declarationList.flags & ts.NodeFlags.Const))
      for (const declaration of statement.declarationList.declarations) bindingNames(declaration.name, names);
  }
  return names;
}

/** Identifiers `fn` reads that it does not declare itself; `null` when it reads `this` or `arguments`. */
function freeNames(fn) {
  const own = declared(fn), free = new Set();
  let impure = false;
  const visit = node => {
    if (impure || ts.isTypeNode(node)) return;
    if (node.kind === ts.SyntaxKind.ThisKeyword) { impure = true; return; }
    if (ts.isIdentifier(node)) {
      const parent = node.parent;
      const named = (ts.isPropertyAccessExpression(parent) && parent.name === node) ||
        (ts.isPropertyAssignment(parent) && parent.name === node) || ts.isJsxAttribute(parent) ||
        ((ts.isJsxOpeningElement(parent) || ts.isJsxSelfClosingElement(parent) || ts.isJsxClosingElement(parent)) &&
          parent.tagName === node && /^[a-z]/.test(node.text));
      if (node.text === 'arguments') impure = true;
      else if (!named && !own.has(node.text)) free.add(node.text);
    }
    ts.forEachChild(node, visit);
  };
  ts.forEachChild(fn, visit);
  return impure ? null : free;
}

/**
 * Marks each `<List renderItem>` whose function reads only its arguments, module constants and
 * pure globals with `__lxRow`, a site id: the runtime may then reuse its rows across renders while
 * `items` is the same array.
 */
export function markStableRows(source, filename, label) {
  if (!source.includes('renderItem')) return source;
  const file = parseSource(filename, source);
  const constants = moduleConstants(file), edits = [];
  const visit = node => {
    if (ts.isJsxAttribute(node) && node.name.getText(file) === 'renderItem' && node.initializer && ts.isJsxExpression(node.initializer)) {
      const expression = node.initializer.expression;
      if (expression && (ts.isArrowFunction(expression) || ts.isFunctionExpression(expression)) && stable(expression))
        edits.push([expression.getStart(file), expression.end, `${label}:${expression.getStart(file)}`]);
    }
    ts.forEachChild(node, visit);
  };
  const stable = fn => {
    const free = freeNames(fn);
    if (!free) return false;
    // A name an enclosing function declares is a capture, whatever the module also declares.
    const enclosing = new Set();
    for (let owner = fn.parent; owner && !ts.isSourceFile(owner); owner = owner.parent)
      if (ts.isFunctionLike(owner)) declared(owner, enclosing);
    return [...free].every(name => !enclosing.has(name) && (constants.has(name) || pureGlobals.has(name)));
  };
  visit(file);
  let output = source;
  for (const [start, end, site] of edits.sort((a, b) => b[0] - a[0]))
    output = `${output.slice(0, start)}Object.assign(${output.slice(start, end)}, { __lxRow: ${JSON.stringify(site)} })${output.slice(end)}`;
  return output;
}
