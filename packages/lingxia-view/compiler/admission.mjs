import ts from 'typescript';
import { authoringRules as rules } from '../src/generated/contract.js';
import { elementKind } from './class-usage.mjs';

const has = (object, key) => Object.prototype.hasOwnProperty.call(object, key);

// The values an expression can take when the source says so: literals and the branches of `?:`
// and `&&`. `undefined` stands for "absent"; null means the source does not say.
function literals(expression) {
  if (!expression) return [true];
  while (ts.isParenthesizedExpression(expression)) expression = expression.expression;
  if (ts.isStringLiteralLike(expression)) return [expression.text];
  if (ts.isNumericLiteral(expression)) return [Number(expression.text)];
  if (expression.kind === ts.SyntaxKind.TrueKeyword) return [true];
  if (expression.kind === ts.SyntaxKind.FalseKeyword) return [false];
  if (ts.isIdentifier(expression) && expression.text === 'undefined') return [undefined];
  if (ts.isConditionalExpression(expression)) {
    const [yes, no] = [literals(expression.whenTrue), literals(expression.whenFalse)];
    return yes && no ? [...yes, ...no] : null;
  }
  if (ts.isBinaryExpression(expression) && expression.operatorToken.kind === ts.SyntaxKind.AmpersandAmpersandToken) {
    const right = literals(expression.right);
    return right ? [undefined, ...right] : null;
  }
  return null;
}

function attributeValues(attribute) {
  const value = attribute.initializer;
  if (!value) return [true];
  return ts.isStringLiteral(value) ? [value.text] : ts.isJsxExpression(value) ? literals(value.expression) : null;
}

/** The schema `rules` a literal JSX element breaks, as `[node, message]`; unknown values pass. */
export function admissionProblems(node, file) {
  const attributes = node.attributes.properties;
  const spread = attributes.some(ts.isJsxSpreadAttribute);
  const named = new Map(attributes.filter(ts.isJsxAttribute).map(attribute => [attribute.name.getText(file), attribute]));
  // Present unless the source says it is always absent or empty.
  const present = name => {
    if (!named.has(name)) return spread;
    const values = attributeValues(named.get(name));
    return !values || values.some(value => value !== undefined && value !== false && !(typeof value === 'string' && !value.trim()));
  };
  const kind = elementKind(node);
  const problems = [];
  const role = named.get('role');
  for (const value of (role && attributeValues(role)) ?? []) {
    if (typeof value !== 'string') continue;
    const only = rules.elementRole[kind];
    if (only && !only.roles.includes(value)) { problems.push([role, `UI_PROP: ${only.error}`]); continue; }
    const broken = (has(rules.role, value) ? rules.role[value] : [])
      .find(rule => (rule.tags && !rule.tags.includes(kind)) || rule.with?.some(name => !present(name)));
    if (broken) problems.push([role, `UI_PROP: ${broken.error}`]);
  }
  return problems;
}
