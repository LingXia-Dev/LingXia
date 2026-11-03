import ts from 'typescript';

// Keys that no utility sets: they stay in `style` even with fixed values.
const styleOnly = new Set(['positionArea', 'animation', 'transition', 'maskImage']);

const at = node => {
  const file = node.getSourceFile();
  const { line, character } = file.getLineAndCharacterOfPosition(node.getStart(file));
  return `${file.fileName}:${line + 1}:${character + 1}`;
};

const literal = node => {
  while (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression(node)) node = node.expression;
  if (ts.isPrefixUnaryExpression(node) && node.operator === ts.SyntaxKind.MinusToken) node = node.operand;
  return ts.isNumericLiteral(node) || ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node);
};

/**
 * `style={{…}}` objects whose every value is a literal with a utility equivalent, as
 * `file:line:col: UI_STYLE_STATIC: …`. Tailwind is the styling dialect; `style` carries computed values.
 */
export function staticStyleWarnings(files) {
  const warnings = [];
  for (const file of files) {
    const visit = node => {
      if (ts.isJsxAttribute(node) && node.name.getText() === 'style' && node.initializer && ts.isJsxExpression(node.initializer)) {
        const object = node.initializer.expression;
        if (object && ts.isObjectLiteralExpression(object) && object.properties.length &&
          object.properties.every(prop => ts.isPropertyAssignment(prop) && !ts.isComputedPropertyName(prop.name) &&
            !styleOnly.has(prop.name.getText().replace(/['"]/g, '')) && literal(prop.initializer)))
          warnings.push(`${at(node)}: UI_STYLE_STATIC: every value here is fixed; write it as Tailwind classes in className and keep style for computed values`);
      }
      ts.forEachChild(node, visit);
    };
    visit(file);
  }
  return warnings;
}
