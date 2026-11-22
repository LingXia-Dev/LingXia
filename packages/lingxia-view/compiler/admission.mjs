import ts from 'typescript';
import { authoringRules as rules, admission, elements, kindOf, commonProps } from '../src/generated/contract.js';
import { elementKind, enclosingTags } from './class-usage.mjs';

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

// Whether every branch of an attribute sets it; a value the source does not fix counts as set.
function alwaysSet(attribute) {
  let expression = attribute.initializer;
  if (expression && ts.isJsxExpression(expression)) expression = expression.expression;
  const sets = node => {
    if (!node) return true;
    while (ts.isParenthesizedExpression(node)) node = node.expression;
    if (ts.isConditionalExpression(node)) return sets(node.whenTrue) && sets(node.whenFalse);
    if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.AmpersandAmpersandToken) return false;
    return !(ts.isIdentifier(node) && node.text === 'undefined') && node.kind !== ts.SyntaxKind.FalseKeyword && node.kind !== ts.SyntaxKind.NullKeyword;
  };
  return sets(expression);
}

function attributeValues(attribute) {
  const value = attribute.initializer;
  if (!value) return [true];
  return ts.isStringLiteral(value) ? [value.text] : ts.isJsxExpression(value) ? literals(value.expression) : null;
}

/** The schema `rules` a literal JSX element always breaks, as `[node, message]`; a value the
 * source does not fix, or one branch that passes, lets the rule through. */
export function admissionProblems(node, file) {
  const attributes = node.attributes.properties;
  const spread = attributes.some(ts.isJsxSpreadAttribute);
  const named = new Map(attributes.filter(ts.isJsxAttribute).map(attribute => [attribute.name.getText(file), attribute]));
  // The values a prop may take: [undefined] when absent, null when the source does not say.
  const values = name => named.has(name) ? attributeValues(named.get(name)) : spread ? null : [undefined];
  const present = name => {
    const list = values(name);
    return !list || list.some(value => value !== undefined && value !== false && !(typeof value === 'string' && !value.trim()));
  };
  const kind = elementKind(node);
  // A run outside <text>, when the nearest enclosing element is written here and is not a run holder.
  const [near] = enclosingTags(node);
  if (admission.tags[kindOf(kind).wire].parent === 'text' && near && has(elements, near) && near !== 'text' && elements[near].children !== 'runs')
    return [[node, `UI_TEXT: <${kind}> belongs inside <text>; wrap it in a <text>`]];
  const only = rules.elementRole[kind];
  const problems = [];
  const run = has(rules.run, kind) ? rules.run[kind] : undefined;
  if (run) for (const [name, attribute] of named)
    if (commonProps.includes(name) && !run.includes(name)) problems.push([attribute, `UI_PROP: <${kind}> is a run of its <text>; put ${name} on the <text>`]);
  const role = named.get('role');
  // null: the source does not fix the role, so no role-gated rule fails.
  const roles = (values('role') ?? [null]).map(value => value === undefined ? only?.implied : value);
  const breaks = (rule, value) => (rule.tags && !rule.tags.includes(kind))
    || (rule.role && value !== null && !rule.role.includes(value))
    || rule.with?.some(name => !present(name))
    || (rule.not && Object.keys(rule.not).some(name => { const list = values(name); return list && list.every(item => rule.not[name].includes(item)); }))
    || rule.any?.every(alternative => breaks(alternative, value))
    // Only a prop written here and set in every branch breaks a `without`.
    || rule.without?.some(name => named.has(name) && alwaysSet(named.get(name)));
  for (const value of (role && attributeValues(role)) ?? []) {
    if (typeof value !== 'string') continue;
    if (only && !only.roles.includes(value)) { problems.push([role, `UI_PROP: ${only.error}`]); continue; }
    const broken = (has(rules.role, value) ? rules.role[value] : []).find(rule => breaks(rule, value));
    if (broken) problems.push([role, `UI_PROP: ${broken.error}`]);
  }
  for (const [name, list] of Object.entries(rules.prop)) {
    const attribute = named.get(name);
    const set = attribute && attributeValues(attribute);
    if (!attribute || (set && set.every(value => value === undefined))) continue;
    const broken = list.find(rule => (!rule.on || rule.on.includes(kind))
      && (!('value' in rule) || (set && set.includes(rule.value)))
      && roles.every(value => breaks(rule, value)));
    if (broken) problems.push([attribute, `UI_PROP: ${broken.error}`]);
  }
  return problems;
}
