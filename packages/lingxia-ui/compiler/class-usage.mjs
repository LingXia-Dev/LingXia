import ts from 'typescript';
import { sourceChecker } from './source.mjs';

// Follow lexical bindings, never identifier spelling: nested components may
// reuse a name for an unrelated value. This analysis does not execute app code.
export function collectClassUsage(file, checker = sourceChecker(file)) {
  const usages = [];
  const strings = (node, seen = new Set()) => {
    if (!node || seen.has(node)) return [];
    seen = new Set(seen).add(node);
    if (ts.isStringLiteralLike(node)) return [node];
    if (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression(node) || ts.isNonNullExpression(node))
      return strings(node.expression, seen);
    if (ts.isConditionalExpression(node)) return [...strings(node.whenTrue, seen), ...strings(node.whenFalse, seen)];
    if (ts.isBinaryExpression(node)) {
      if (node.operatorToken.kind === ts.SyntaxKind.AmpersandAmpersandToken) return strings(node.right, seen);
      if ([ts.SyntaxKind.BarBarToken, ts.SyntaxKind.QuestionQuestionToken].includes(node.operatorToken.kind))
        return [...strings(node.left, seen), ...strings(node.right, seen)];
    }
    if (ts.isIdentifier(node)) {
      let symbol = checker.getSymbolAtLocation(node);
      if (symbol && (symbol.flags & ts.SymbolFlags.Alias)) symbol = checker.getAliasedSymbol(symbol);
      const declaration = symbol?.valueDeclaration;
      if (declaration && ts.isVariableDeclaration(declaration) &&
          (declaration.parent.flags & ts.NodeFlags.Const)) return strings(declaration.initializer, seen);
    }
    if (ts.isPropertyAccessExpression(node)) {
      const declaration = checker.getSymbolAtLocation(node.name)?.valueDeclaration;
      if (declaration && ts.isPropertyAssignment(declaration)) return strings(declaration.initializer, seen);
      if (declaration && ts.isShorthandPropertyAssignment(declaration))
        return strings(checker.getShorthandAssignmentValueSymbol(declaration)?.valueDeclaration?.initializer, seen);
    }
    if (ts.isElementAccessExpression(node)) {
      const type = checker.getTypeAtLocation(node.expression);
      const properties = ts.isStringLiteralLike(node.argumentExpression)
        ? [type.getProperty(node.argumentExpression.text)] : type.getProperties();
      return properties.flatMap(symbol => {
        const declaration = symbol?.valueDeclaration;
        if (declaration && ts.isShorthandPropertyAssignment(declaration))
          return strings(checker.getShorthandAssignmentValueSymbol(declaration)?.valueDeclaration?.initializer, seen);
        return declaration && ts.isPropertyAssignment(declaration) ? strings(declaration.initializer, seen) : [];
      });
    }
    return [];
  };
  const record = (expression, tag) => {
    for (const literal of strings(expression)) {
      const owner = literal.getSourceFile();
      const { line, character } = owner.getLineAndCharacterOfPosition(literal.getStart(owner));
      usages.push({ value: literal.text, tag, location: `${owner.fileName}:${line + 1}:${character + 1}` });
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
