import ts from 'typescript';
import { sourceChecker } from './source.mjs';

// Follow lexical bindings, never identifier spelling: nested components may
// reuse a name for an unrelated value. This analysis does not execute app code.
/** The string literals an expression can evaluate to, through const bindings and object maps. */
export function stringLiterals(checker, node) {
  return trace(checker, node, false).pieces.map(piece => piece.node);
}

/** The class text an expression can evaluate to: `stringLiterals`, plus template pieces and the
 * arguments of `cx` from `@lingxia/view`. Each piece is `{ text, node }`. `known(node)` may name a
 * condition's or map key's value (`{ value }`) so only the branch or key it selects is followed. */
export function classPieces(checker, node, known = () => undefined) {
  return trace(checker, node, true, known).pieces;
}

/** `classPieces`, plus `untraced`: each sub-expression whose value the build cannot follow to
 * literals, as `{ node, reason }`. Tracing never throws; what it cannot follow is reported here. */
export function traceClasses(checker, node, known = () => undefined) {
  return trace(checker, node, true, known);
}

function trace(checker, root, classes, known = () => undefined) {
  const pieces = [], untraced = [];
  const miss = (node, reason) => untraced.push({ node, reason });
  // The receiver is not a written object: its type's declarations are all the build knows.
  const typeMembers = (node, key, next) => {
    if (ts.isElementAccessExpression(node)) {
      const type = checker.getTypeAtLocation(node.expression);
      if (checker.isArrayType(type) || checker.isTupleType(type)) return miss(node, `${quote(node)} is an array element, which the build does not follow`);
    }
    const symbols = ts.isPropertyAccessExpression(node) ? [checker.getSymbolAtLocation(node.name)]
      : key !== undefined ? [checker.getTypeAtLocation(node.expression).getProperty(key)] : checker.getTypeAtLocation(node.expression).getProperties();
    let traced = false;
    for (const symbol of symbols) {
      const declaration = symbol?.valueDeclaration;
      if (declaration && ts.isPropertyAssignment(declaration)) { next(declaration.initializer); traced = true; }
      else if (declaration && ts.isShorthandPropertyAssignment(declaration)) { next(declaration.name); traced = true; }
    }
    const declaration = symbols.length === 1 ? symbols[0]?.valueDeclaration ?? symbols[0]?.declarations?.[0] : undefined;
    miss(node, traced ? `${quote(node)} is read from ${quote(node.expression)}, which the build cannot follow to a written object; only the members its type declares are checked`
      : declaration && (ts.isPropertySignature(declaration) || ts.isPropertyDeclaration(declaration)) ? `${quote(node)} is known only by its type, not by a value the build can read`
      : `${quote(node)} is read from ${quote(node.expression)}, which the build cannot follow to a written object`);
  };
  const visit = (node, seen) => {
    if (!node) return;
    if (seen.has(node)) return miss(node, `${quote(node)} refers to itself`);
    seen = new Set(seen).add(node);
    const next = child => visit(child, seen);
    if (ts.isStringLiteralLike(node)) return void pieces.push({ text: node.text, node });
    if (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression(node) || ts.isNonNullExpression(node))
      return next(node.expression);
    if (empty(checker, node)) return;
    if (ts.isConditionalExpression(node)) {
      const test = known(node.condition);
      if (test) return next(test.value ? node.whenTrue : node.whenFalse);
      next(node.whenTrue);
      return next(node.whenFalse);
    }
    if (ts.isBinaryExpression(node)) {
      const left = known(node.left);
      const operator = node.operatorToken.kind;
      if (operator === ts.SyntaxKind.AmpersandAmpersandToken) return left && !left.value ? undefined : next(node.right);
      if (left && operator === ts.SyntaxKind.BarBarToken) return next(left.value ? node.left : node.right);
      if (operator === ts.SyntaxKind.BarBarToken || operator === ts.SyntaxKind.QuestionQuestionToken) {
        next(node.left);
        return next(node.right);
      }
      return miss(node, `${quote(node)} is computed with \`${ts.tokenToString(operator)}\` at run time`);
    }
    if (ts.isIdentifier(node)) {
      const symbol = valueSymbol(checker, node);
      const declaration = symbol?.valueDeclaration;
      if (declaration && ts.isVariableDeclaration(declaration) && (declaration.parent.flags & ts.NodeFlags.Const) && declaration.initializer)
        return next(declaration.initializer);
      return miss(node, bindingReason(node, symbol, declaration));
    }
    if (ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node)) {
      const owners = objects(checker, node.expression, known);
      const key = memberKey(node, known);
      if (owners) {
        for (const owner of owners) {
          const values = key === undefined ? allMembers(owner) : [member(owner, key)];
          for (const value of values)
            if (value === undefined) miss(node, `${quote(node)} reads a member the object does not write out`);
            else next(value);
        }
        return;
      }
      return typeMembers(node, key, next);
    }
    if (!classes) return;
    if (ts.isTemplateExpression(node)) {
      pieces.push(...templatePieces(node, miss));
      for (const span of node.templateSpans) next(span.expression);
      return;
    }
    if (ts.isCallExpression(node) && isCx(checker, node.expression)) {
      for (const argument of node.arguments) cxPieces(argument, next, piece => pieces.push(piece), miss, known);
      return;
    }
    if (ts.isCallExpression(node)) return miss(node, `${quote(node)} is the result of a call`);
    if (ts.isObjectLiteralExpression(node) || ts.isArrayLiteralExpression(node)) return miss(node, `${quote(node)} is ${ts.isArrayLiteralExpression(node) ? 'an array' : 'an object'}, not class text`);
    return miss(node, `${quote(node)} is ${describe(node)}, which the build does not follow`);
  };
  try {
    visit(root, new Set());
  } catch (error) {
    // Whatever the tracer cannot follow is reported, never a build error.
    miss(root, `the class tracer failed on ${quote(root)}: ${error?.message ?? error}`);
  }
  return { pieces, untraced };
}

/** The written object literals `node` can be, or undefined when any of them is not one. */
function objects(checker, node, known, seen = new Set()) {
  while (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression(node) || ts.isNonNullExpression(node)) node = node.expression;
  if (!node || seen.has(node)) return undefined;
  seen = new Set(seen).add(node);
  if (ts.isObjectLiteralExpression(node)) return [node];
  if (ts.isConditionalExpression(node)) {
    const test = known(node.condition);
    if (test) return objects(checker, test.value ? node.whenTrue : node.whenFalse, known, seen);
    const both = [objects(checker, node.whenTrue, known, seen), objects(checker, node.whenFalse, known, seen)];
    return both.every(Boolean) ? both.flat() : undefined;
  }
  if (ts.isIdentifier(node)) {
    const declaration = valueSymbol(checker, node)?.valueDeclaration;
    return declaration && ts.isVariableDeclaration(declaration) && (declaration.parent.flags & ts.NodeFlags.Const) && declaration.initializer
      ? objects(checker, declaration.initializer, known, seen) : undefined;
  }
  if (ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node)) {
    const owners = objects(checker, node.expression, known, seen);
    if (!owners) return undefined;
    const key = memberKey(node, known);
    const found = [];
    for (const owner of owners)
      for (const value of key === undefined ? allMembers(owner) : [member(owner, key)]) {
        const inner = value && objects(checker, value, known, seen);
        if (!inner) return undefined;
        found.push(...inner);
      }
    return found;
  }
  return undefined;
}

/** The binding an identifier reads: through imports, and to the variable a shorthand member names. */
function valueSymbol(checker, node) {
  let symbol = ts.isShorthandPropertyAssignment(node.parent) && node.parent.name === node
    ? checker.getShorthandAssignmentValueSymbol(node.parent) : checker.getSymbolAtLocation(node);
  if (symbol && (symbol.flags & ts.SymbolFlags.Alias)) symbol = checker.getAliasedSymbol(symbol);
  return symbol;
}

/** The member name an access reads, when the build knows it. */
function memberKey(node, known) {
  if (ts.isPropertyAccessExpression(node)) return node.name.text;
  if (ts.isStringLiteralLike(node.argumentExpression)) return node.argumentExpression.text;
  const key = known(node.argumentExpression);
  return key && typeof key.value === 'string' ? key.value : undefined;
}

/** The value expression `owner` writes for `key`; undefined when it does not write one out. */
function member(owner, key) {
  for (const property of [...owner.properties].reverse()) {
    if (ts.isSpreadAssignment(property)) return undefined;
    if (!property.name || propertyName(property.name) !== key) continue;
    if (ts.isPropertyAssignment(property)) return property.initializer;
    if (ts.isShorthandPropertyAssignment(property)) return property.name;
    return undefined;
  }
  return undefined;
}

/** Every member value `owner` writes out, with undefined for one it does not. */
function allMembers(owner) {
  return owner.properties.map(property => ts.isPropertyAssignment(property) ? property.initializer
    : ts.isShorthandPropertyAssignment(property) ? property.name : undefined);
}

function propertyName(name) {
  if (ts.isIdentifier(name) || ts.isStringLiteralLike(name) || ts.isNumericLiteral(name)) return name.text;
  if (ts.isComputedPropertyName(name) && ts.isStringLiteralLike(name.expression)) return name.expression.text;
  return undefined;
}

/** `undefined`, `null` and `false`: no class at all. */
function empty(checker, node) {
  if (node.kind === ts.SyntaxKind.NullKeyword || node.kind === ts.SyntaxKind.FalseKeyword) return true;
  if (!ts.isIdentifier(node) || node.text !== 'undefined') return false;
  const symbol = checker.getSymbolAtLocation(node);
  return !symbol || !symbol.valueDeclaration;
}

/** `cx` arguments: class text, arrays of it, and `{ 'class names': condition }` maps. */
function cxPieces(node, next, push, miss, known = () => undefined) {
  while (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression(node)) node = node.expression;
  if (ts.isArrayLiteralExpression(node)) {
    for (const element of node.elements) cxPieces(element, next, push, miss, known);
    return;
  }
  if (ts.isObjectLiteralExpression(node)) {
    for (const property of node.properties) {
      const on = ts.isPropertyAssignment(property) ? known(property.initializer) : undefined;
      if (on && !on.value) continue;
      const name = property.name;
      if (name && (ts.isStringLiteralLike(name) || ts.isIdentifier(name))) push({ text: name.text, node: name });
      else if (name && ts.isComputedPropertyName(name)) next(name.expression);
      else miss(property, `${quote(property)} in a cx map is ${describe(property)}, which the build does not follow`);
    }
    return;
  }
  next(node);
}

/** A template's literal text, less any class name an interpolation joins: `px-${n}` checks nothing. */
function templatePieces(node, miss) {
  const parts = [node.head, ...node.templateSpans.map(span => span.literal)];
  node.templateSpans.forEach(({ expression }, index) => {
    const before = parts[index].text, after = parts[index + 1].text;
    const last = index === node.templateSpans.length - 1;
    if ((before && !/\s$/.test(before)) || (after && !/^\s/.test(after)) || (!after && !last)) miss(expression, joinReason(expression));
  });
  return parts.map((part, index) => {
    const words = part.text.split(/(\s+)/);
    if (index > 0 && !/^\s/.test(part.text)) words[0] = '';
    if (index < parts.length - 1 && !/\s$/.test(part.text)) words[words.length - 1] = '';
    return { text: words.join(''), node: part };
  });
}

const joinReason = expression => `${quote(expression)} is joined into a class name at run time`;

/** `cx` imported from `@lingxia/view`, under any local name. */
function isCx(checker, callee) {
  if (!ts.isIdentifier(callee)) return false;
  const declaration = checker.getSymbolAtLocation(callee)?.declarations?.[0];
  if (!declaration || !ts.isImportSpecifier(declaration)) return false;
  const imported = (declaration.propertyName ?? declaration.name).text;
  const module = declaration.parent.parent.parent.moduleSpecifier;
  return imported === 'cx' && ts.isStringLiteral(module) && module.text === '@lingxia/view';
}

function quote(node) {
  const text = node.getText(node.getSourceFile()).replace(/\s+/g, ' ');
  return `\`${text.length > 60 ? `${text.slice(0, 57)}...` : text}\``;
}

function describe(node) {
  const words = ts.SyntaxKind[node.kind].replace(/(Expression|Literal|Assignment|Keyword)$/, ' $1').replace(/([a-z])([A-Z])/g, '$1 $2').toLowerCase().trim();
  return `${/^[aeiou]/.test(words) ? 'an' : 'a'} ${words}`;
}

function bindingReason(node, symbol, declaration) {
  const name = quote(node);
  if (!symbol) return `${name} cannot be resolved`;
  declaration ??= symbol.declarations?.[0];
  if (!declaration) return `${name} has no declaration the build can read`;
  if (ts.isParameter(declaration)) return `${name} is a parameter; its value comes from each caller`;
  if (ts.isBindingElement(declaration)) {
    let owner = declaration;
    while (ts.isBindingElement(owner) || ts.isObjectBindingPattern(owner) || ts.isArrayBindingPattern(owner)) owner = owner.parent;
    if (ts.isParameter(owner)) return `${name} is destructured from a parameter; its value comes from each caller`;
    return `${name} is destructured, which the build does not follow`;
  }
  if (ts.isVariableDeclaration(declaration)) {
    if (!(declaration.parent.flags & ts.NodeFlags.Const)) return `${name} is a \`${declaration.parent.flags & ts.NodeFlags.Let ? 'let' : 'var'}\` binding, which can be reassigned`;
    return `${name} is declared without a value`;
  }
  return `${name} is ${describe(declaration).replace(/ declaration$/, '')}, not a constant`;
}

/** The intrinsic elements around `element`, innermost first, within the function that builds it;
 * a `.map(item => …)` callback inside JSX is part of that function. */
export function enclosingTags(element) {
  const tags = [];
  let node = (ts.isJsxOpeningElement(element) ? element.parent : element).parent;
  while (node && !ts.isSourceFile(node)) {
    if (ts.isJsxElement(node)) tags.push(node.openingElement.tagName.getText());
    if (ts.isFunctionLike(node) && !(ts.isCallExpression(node.parent) && node.parent.arguments.includes(node))) break;
    node = node.parent;
  }
  return tags;
}

/** The element kind a tag lowers to: a `<link>` not known to sit in `<text>` is a block link. */
export function elementKind(element) {
  const tag = element.tagName.getText();
  if (tag !== 'link') return tag;
  const [parent] = enclosingTags(element);
  return parent === 'text' || parent === 'span' || parent === 'link' ? 'link' : 'link-box';
}

function location(node) {
  const owner = node.getSourceFile();
  const { line, character } = owner.getLineAndCharacterOfPosition(node.getStart(owner));
  return `${owner.fileName}:${line + 1}:${character + 1}`;
}

/** Every `className` in `file` as checked usages; what the build cannot follow goes to `untraced`. */
export function collectClassUsage(file, checker = sourceChecker(file), untraced = []) {
  const usages = [];
  const record = (expression, tag, focusable) => {
    const result = traceClasses(checker, expression);
    for (const { text, node } of result.pieces) usages.push({ value: text, tag, focusable, location: location(node) });
    untraced.push(...result.untraced);
  };
  // An element its props make a keyboard stop: a drag element, or one that hears the pointer and
  // reports a value. A spread may carry either.
  const takesFocus = attributes => {
    const names = new Set(attributes.properties.filter(ts.isJsxAttribute).map(prop => prop.name.getText(file)));
    return names.has('drag') || (names.has('onPointer') && names.has('aria-valuenow')) || attributes.properties.some(ts.isJsxSpreadAttribute);
  };
  const visit = node => {
    if (ts.isJsxAttribute(node) && node.name.getText(file) === 'className' && node.initializer) {
      const expression = ts.isJsxExpression(node.initializer) ? node.initializer.expression : node.initializer;
      record(expression, elementKind(node.parent.parent), takesFocus(node.parent));
    }
    if (ts.isJsxSpreadAttribute(node)) {
      const attributes = node.parent.properties;
      const overridden = attributes.slice(attributes.indexOf(node) + 1)
        .some(prop => ts.isJsxAttribute(prop) && prop.name.getText(file) === 'className');
      // Only a declared `className` can carry classes; an index signature (`Record<string, …>`) is not one.
      const property = overridden ? undefined : checker.getTypeAtLocation(node.expression).getProperty('className');
      if (property) {
        const tag = elementKind(node.parent.parent);
        let typed = false;
        for (const declaration of property.declarations ?? []) {
          if (ts.isPropertyAssignment(declaration)) record(declaration.initializer, tag, true);
          else if (ts.isShorthandPropertyAssignment(declaration))
            record(checker.getShorthandAssignmentValueSymbol(declaration)?.valueDeclaration?.initializer, tag, true);
          else typed = true;
        }
        if (typed || !property.declarations?.length)
          untraced.push({ node: node.expression, reason: `the spread ${quote(node.expression)} declares a className known only by its type` });
      }
    }
    ts.forEachChild(node, visit);
  };
  try {
    visit(file);
  } catch (error) {
    untraced.push({ node: file, reason: `the class tracer failed in this file: ${error?.message ?? error}` });
  }
  return usages;
}

/** Class usages across `files`, and each place the build cannot follow a className to literals,
 * as `{ location, expression, reason }`, once per place and reason. */
export function collectClassAnalysis(files, checker) {
  const untraced = [];
  const usages = files.flatMap(file => collectClassUsage(file, checker, untraced));
  const seen = new Set(), places = [];
  for (const { node, reason } of untraced) {
    const at = location(node), key = `${at}\0${reason}`;
    if (seen.has(key)) continue;
    seen.add(key);
    places.push({ location: at, expression: node.getText(node.getSourceFile()).replace(/\s+/g, ' '), reason });
  }
  return { usages, untraced: places };
}
