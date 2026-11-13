import ts from 'typescript';
import { sourceChecker } from './source.mjs';

// Follow lexical bindings, never identifier spelling: nested components may
// reuse a name for an unrelated value. This analysis does not execute app code.
/** The string literals an expression can evaluate to, through const bindings and object maps. */
export function stringLiterals(checker, node) {
  return pieces(checker, node, new Set(), false).map(piece => piece.node);
}

/** The class text an expression can evaluate to: `stringLiterals`, plus template pieces and the
 * arguments of `cx` from `@lingxia/view`. Each piece is `{ text, node }`. `known(node)` may name a
 * condition's or map key's value (`{ value }`) so only the branch or key it selects is followed. */
export function classPieces(checker, node, known = () => undefined) {
  return pieces(checker, node, new Set(), true, known);
}

function pieces(checker, node, seen, classes, known = () => undefined) {
  if (!node || seen.has(node)) return [];
  seen = new Set(seen).add(node);
  const next = child => pieces(checker, child, seen, classes, known);
  if (ts.isStringLiteralLike(node)) return [{ text: node.text, node }];
  if (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression(node) || ts.isNonNullExpression(node))
    return next(node.expression);
  if (ts.isConditionalExpression(node)) {
    const test = known(node.condition);
    if (test) return next(test.value ? node.whenTrue : node.whenFalse);
    return [...next(node.whenTrue), ...next(node.whenFalse)];
  }
  if (ts.isBinaryExpression(node)) {
    const left = known(node.left);
    if (node.operatorToken.kind === ts.SyntaxKind.AmpersandAmpersandToken) return left && !left.value ? [] : next(node.right);
    if (left && node.operatorToken.kind === ts.SyntaxKind.BarBarToken) return next(left.value ? node.left : node.right);
    if ([ts.SyntaxKind.BarBarToken, ts.SyntaxKind.QuestionQuestionToken].includes(node.operatorToken.kind))
      return [...next(node.left), ...next(node.right)];
  }
  if (ts.isIdentifier(node)) {
    let symbol = checker.getSymbolAtLocation(node);
    if (symbol && (symbol.flags & ts.SymbolFlags.Alias)) symbol = checker.getAliasedSymbol(symbol);
    const declaration = symbol?.valueDeclaration;
    if (declaration && ts.isVariableDeclaration(declaration) &&
        (declaration.parent.flags & ts.NodeFlags.Const)) return next(declaration.initializer);
  }
  if (ts.isPropertyAccessExpression(node)) {
    const declaration = checker.getSymbolAtLocation(node.name)?.valueDeclaration;
    if (declaration && ts.isPropertyAssignment(declaration)) return next(declaration.initializer);
    if (declaration && ts.isShorthandPropertyAssignment(declaration))
      return next(checker.getShorthandAssignmentValueSymbol(declaration)?.valueDeclaration?.initializer);
  }
  if (ts.isElementAccessExpression(node)) {
    const type = checker.getTypeAtLocation(node.expression);
    const key = ts.isStringLiteralLike(node.argumentExpression) ? { value: node.argumentExpression.text } : known(node.argumentExpression);
    const properties = key && typeof key.value === 'string' ? [type.getProperty(key.value)] : type.getProperties();
    return properties.flatMap(symbol => {
      const declaration = symbol?.valueDeclaration;
      if (declaration && ts.isShorthandPropertyAssignment(declaration))
        return next(checker.getShorthandAssignmentValueSymbol(declaration)?.valueDeclaration?.initializer);
      return declaration && ts.isPropertyAssignment(declaration) ? next(declaration.initializer) : [];
    });
  }
  if (!classes) return [];
  if (ts.isTemplateExpression(node)) return [...templatePieces(node), ...node.templateSpans.flatMap(span => next(span.expression))];
  if (ts.isCallExpression(node) && isCx(checker, node.expression)) return node.arguments.flatMap(argument => cxPieces(argument, next, known));
  return [];
}

/** `cx` arguments: class text, arrays of it, and `{ 'class names': condition }` maps. */
function cxPieces(node, next, known = () => undefined) {
  while (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression(node)) node = node.expression;
  if (ts.isArrayLiteralExpression(node)) return node.elements.flatMap(element => cxPieces(element, next, known));
  if (ts.isObjectLiteralExpression(node))
    return node.properties.flatMap(property => {
      const on = ts.isPropertyAssignment(property) ? known(property.initializer) : undefined;
      if (on && !on.value) return [];
      const name = property.name;
      if (name && (ts.isStringLiteralLike(name) || ts.isIdentifier(name))) return [{ text: name.text, node: name }];
      return name && ts.isComputedPropertyName(name) ? next(name.expression) : [];
    });
  return next(node);
}

/** A template's literal text, less any class name an interpolation joins: `px-${n}` checks nothing. */
function templatePieces(node) {
  const parts = [node.head, ...node.templateSpans.map(span => span.literal)];
  return parts.map((part, index) => {
    const words = part.text.split(/(\s+)/);
    if (index > 0 && !/^\s/.test(part.text)) words[0] = '';
    if (index < parts.length - 1 && !/\s$/.test(part.text)) words[words.length - 1] = '';
    return { text: words.join(''), node: part };
  });
}

/** `cx` imported from `@lingxia/view`, under any local name. */
function isCx(checker, callee) {
  if (!ts.isIdentifier(callee)) return false;
  const declaration = checker.getSymbolAtLocation(callee)?.declarations?.[0];
  if (!declaration || !ts.isImportSpecifier(declaration)) return false;
  const imported = (declaration.propertyName ?? declaration.name).text;
  const module = declaration.parent.parent.parent.moduleSpecifier;
  return imported === 'cx' && ts.isStringLiteral(module) && module.text === '@lingxia/view';
}

export function collectClassUsage(file, checker = sourceChecker(file)) {
  const usages = [];
  const record = (expression, tag) => {
    for (const { text, node } of classPieces(checker, expression)) {
      const owner = node.getSourceFile();
      const { line, character } = owner.getLineAndCharacterOfPosition(node.getStart(owner));
      usages.push({ value: text, tag, location: `${owner.fileName}:${line + 1}:${character + 1}` });
    }
  };
  const visit = node => {
    if (ts.isJsxAttribute(node) && node.name.getText(file) === 'className' && node.initializer) {
      const expression = ts.isJsxExpression(node.initializer) ? node.initializer.expression : node.initializer;
      record(expression, node.parent.parent.tagName.getText(file));
    }
    if (ts.isJsxSpreadAttribute(node)) {
      const attributes = node.parent.properties;
      const overridden = attributes.slice(attributes.indexOf(node) + 1)
        .some(prop => ts.isJsxAttribute(prop) && prop.name.getText(file) === 'className');
      if (!overridden) {
        const property = checker.getTypeAtLocation(node.expression).getProperty('className');
        for (const declaration of property?.declarations ?? []) {
          const tag = node.parent.parent.tagName.getText(file);
          if (ts.isPropertyAssignment(declaration)) record(declaration.initializer, tag);
          if (ts.isShorthandPropertyAssignment(declaration))
            record(checker.getShorthandAssignmentValueSymbol(declaration)?.valueDeclaration?.initializer, tag);
        }
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  return usages;
}
