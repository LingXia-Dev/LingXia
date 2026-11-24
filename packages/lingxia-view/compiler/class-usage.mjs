import ts from 'typescript';

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
 * literals, as `{ node, reason }`. With `calls` (`callIndex`), parameters are followed to the
 * arguments of every call and element that uses their function. Tracing never throws. */
export function traceClasses(checker, node, known = () => undefined, calls) {
  return trace(checker, node, true, known, calls);
}

const NO_KNOWN = () => undefined;
const MAX_COMBINATIONS = 64;
// Array methods whose callback's first parameter is an element of the array.
const ELEMENT_CALLBACKS = new Set(['map', 'forEach', 'filter', 'flatMap', 'find', 'findLast', 'some', 'every']);
// Array methods whose result holds only the array's own elements.
const SAME_ELEMENTS = new Set(['filter', 'slice', 'sort', 'reverse', 'toSorted', 'toReversed', 'concat', 'flat']);

function trace(checker, root, classes, known = NO_KNOWN, calls) {
  const pieces = [], untraced = [];
  const miss = (node, reason, built = false) => untraced.push({ node, reason, built });
  const flow = calls?.flow ?? new Flow(checker, calls);
  const visit = (node, seen, k) => {
    if (!node) return;
    // A value met again on its own path adds nothing new: a component forwarding to itself.
    if (seen.has(node)) return;
    seen = new Set(seen).add(node);
    const next = (child, nextKnown = k) => visit(child, seen, nextKnown);
    const elsewhere = child => visit(child, seen, NO_KNOWN);
    // `cx` and a template take numbers as class text.
    if (ts.isStringLiteralLike(node) || ts.isNumericLiteral(node)) return void pieces.push({ text: node.text, node });
    if (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression(node) || ts.isNonNullExpression(node) || ts.isTypeAssertionExpression(node))
      return next(node.expression);
    if (empty(checker, node)) return;
    if (ts.isConditionalExpression(node)) {
      const test = k(node.condition);
      if (test) return next(test.value ? node.whenTrue : node.whenFalse);
      next(node.whenTrue);
      return next(node.whenFalse);
    }
    if (ts.isBinaryExpression(node)) {
      const left = k(node.left);
      const operator = node.operatorToken.kind;
      if (operator === ts.SyntaxKind.AmpersandAmpersandToken) return left && !left.value ? undefined : next(node.right);
      if (left && operator === ts.SyntaxKind.BarBarToken) return next(left.value ? node.left : node.right);
      if (operator === ts.SyntaxKind.BarBarToken || operator === ts.SyntaxKind.QuestionQuestionToken) {
        next(node.left);
        return next(node.right);
      }
      return miss(node, `${quote(node)} is computed with \`${ts.tokenToString(operator)}\` at run time`, true);
    }
    if (ts.isIdentifier(node) || ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node) || (ts.isCallExpression(node) && !isCx(checker, node.expression))) {
      if (!classes && ts.isCallExpression(node)) return;
      const found = flow.values(node, k);
      if (found.why) return miss(found.at ?? node, found.why, found.built);
      for (const value of found.values) (value.crossed ? elsewhere : next)(value.node);
      return;
    }
    if (!classes) return;
    if (ts.isTemplateExpression(node)) return template(node, k);
    if (ts.isCallExpression(node)) {
      for (const argument of node.arguments) cx(argument, seen, k);
      return;
    }
    if (ts.isObjectLiteralExpression(node) || ts.isArrayLiteralExpression(node)) return miss(node, `${quote(node)} is ${ts.isArrayLiteralExpression(node) ? 'an array' : 'an object'}, not class text`);
    if (ts.isJsxAttributes(node)) return miss(node, `${quote(node)} is a props object, not class text`);
    return miss(node, `${quote(node)} is ${describe(node)}, which the build does not follow`);
  };

  // `cx` arguments: class text, arrays of it, and `{ 'class names': condition }` maps, also by name.
  const cx = (node, seen, k) => {
    node = unwrap(node);
    if (!node || empty(checker, node)) return;
    if (ts.isArrayLiteralExpression(node)) {
      for (const element of node.elements)
        if (ts.isSpreadElement(element)) cx(element.expression, seen, k);
        else cx(element, seen, k);
      return;
    }
    if (ts.isObjectLiteralExpression(node)) {
      for (const property of node.properties) {
        const on = ts.isPropertyAssignment(property) ? k(property.initializer) : undefined;
        if (on && !on.value) continue;
        const name = property.name;
        if (name && (ts.isStringLiteralLike(name) || ts.isIdentifier(name) || ts.isNumericLiteral(name))) pieces.push({ text: name.text, node: name });
        else if (name && ts.isComputedPropertyName(name)) visit(name.expression, seen, k);
        else if (ts.isSpreadAssignment(property)) cx(property.expression, seen, k);
        else miss(property, `${quote(property)} in a cx map is ${describe(property)}, which the build does not follow`);
      }
      return;
    }
    if (ts.isConditionalExpression(node)) {
      const test = k(node.condition);
      if (test) return cx(test.value ? node.whenTrue : node.whenFalse, seen, k);
      cx(node.whenTrue, seen, k);
      return cx(node.whenFalse, seen, k);
    }
    if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.AmpersandAmpersandToken) return cx(node.right, seen, k);
    if (ts.isBinaryExpression(node) && [ts.SyntaxKind.BarBarToken, ts.SyntaxKind.QuestionQuestionToken].includes(node.operatorToken.kind)) {
      cx(node.left, seen, k);
      return cx(node.right, seen, k);
    }
    if (ts.isIdentifier(node) || ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node)) {
      // A map or array bound elsewhere is still a cx argument; plain class text goes through `visit`.
      const found = flow.values(node, k);
      if (!found.why && found.values.every(({ node: value }) => isLiteralContainer(unwrap(value)))) {
        for (const value of found.values) cx(value.node, seen, value.crossed ? NO_KNOWN : k);
        return;
      }
    }
    visit(node, seen, k);
  };

  // A template whose holes join words expands to every combination when each hole is a known set.
  const template = (node, k) => {
    const parts = [node.head, ...node.templateSpans.map(span => span.literal)];
    const joins = node.templateSpans.map(({ expression }, index) => {
      const before = parts[index].text, after = parts[index + 1].text;
      const last = index === node.templateSpans.length - 1;
      return (before && !/\s$/.test(before)) || (!before && index > 0) || (after && !/^\s/.test(after)) || (!after && !last);
    });
    if (joins.some(Boolean)) {
      // Only holes that join words need their values; a hole between words is traced on its own.
      const sets = node.templateSpans.map(({ expression }, index) => joins[index] ? texts(checker, expression, k, calls) : [' ']);
      const count = sets.reduce((product, set) => product * (set ? set.length : Infinity), 1);
      if (sets.every(Boolean) && count <= MAX_COMBINATIONS) {
        let combos = [node.head.text];
        node.templateSpans.forEach((span, index) => { combos = combos.flatMap(prefix => sets[index].map(text => prefix + text + span.literal.text)); });
        for (const text of new Set(combos)) pieces.push({ text, node });
        node.templateSpans.forEach(({ expression }, index) => { if (!joins[index]) visit(expression, new Set([node]), k); });
        return;
      }
      node.templateSpans.forEach(({ expression }, index) => {
        if (!joins[index]) return;
        miss(expression, sets[index] && count > MAX_COMBINATIONS
          ? `${quote(node)} expands to ${count} class names (limit ${MAX_COMBINATIONS})`
          : `${quote(expression)} is joined into a class name at run time${sets[index] ? '' : ', and its values are not known to the build'}`, true);
      });
    }
    for (const [index, part] of parts.entries()) {
      const words = part.text.split(/(\s+)/);
      if (index > 0 && !/^\s/.test(part.text)) words[0] = '';
      if (index < parts.length - 1 && !/\s$/.test(part.text)) words[words.length - 1] = '';
      pieces.push({ text: words.join(''), node: part });
    }
    node.templateSpans.forEach(({ expression }, index) => { if (!joins[index]) visit(expression, new Set([node]), k); });
  };

  try {
    visit(root, new Set(), known);
  } catch (error) {
    // Whatever the tracer cannot follow is reported, never a build error.
    miss(root, `the class tracer failed on ${quote(root)}: ${error?.message ?? error}`);
  }
  return { pieces, untraced };
}

/** The texts a template hole can be: strings and numbers, or undefined when any value is unknown. */
function texts(checker, node, known, calls) {
  const out = new Set();
  const flow = calls?.flow ?? new Flow(checker, calls);
  const walk = (node, seen, k) => {
    node = unwrap(node);
    if (!node || seen.has(node)) return true;
    seen = new Set(seen).add(node);
    if (ts.isStringLiteralLike(node) || ts.isNumericLiteral(node)) return out.add(node.text), true;
    if (ts.isConditionalExpression(node)) {
      const test = k(node.condition);
      if (test) return walk(test.value ? node.whenTrue : node.whenFalse, seen, k);
      return walk(node.whenTrue, seen, k) && walk(node.whenFalse, seen, k);
    }
    if (ts.isIdentifier(node) || ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node) || ts.isCallExpression(node)) {
      const found = flow.values(node, k);
      return !found.why && found.values.every(value => walk(value.node, seen, value.crossed ? NO_KNOWN : k));
    }
    return false;
  };
  return walk(node, new Set(), known) ? [...out] : undefined;
}

const isLiteralContainer = node => node && (ts.isObjectLiteralExpression(node) || ts.isArrayLiteralExpression(node));

function unwrap(node) {
  while (node && (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression(node) || ts.isNonNullExpression(node) || ts.isTypeAssertionExpression(node)))
    node = node.expression;
  return node;
}

/** `undefined`, `null` and `false`: no class at all. */
function empty(checker, node) {
  if (node.kind === ts.SyntaxKind.NullKeyword || node.kind === ts.SyntaxKind.FalseKeyword) return true;
  if (ts.isVoidExpression(node)) return true;
  if (!ts.isIdentifier(node) || node.text !== 'undefined') return false;
  const symbol = checker.getSymbolAtLocation(node);
  dependOn(symbol);
  return !symbol || !symbol.valueDeclaration;
}

/**
 * Where values flow from: a binding to what initialises it, a parameter to the arguments of each
 * call or element that uses its function, a member read to the members of the written objects it
 * reads, a call to its function's returns. Each answer is `{ values: [{ node, crossed }] }` or
 * `{ why, at }`; `crossed` marks a value from another function, where the reader's conditions mean nothing.
 */
class Flow {
  constructor(checker, calls) {
    this.checker = checker;
    this.calls = calls;
    // A parameter's values are the same for every read; one met again while being worked out
    // (a part forwarding to itself) answers nothing new, and what depended on it is not kept.
    this.known = new Map();
    this.active = new Set();
    this.cyclic = false;
  }

  values(node, known, seen = new Set()) {
    node = unwrap(node);
    if (seen.has(node)) return { values: [] };
    seen = new Set(seen).add(node);
    const { checker } = this;
    if (ts.isIdentifier(node)) {
      if (empty(checker, node)) return { values: [] };
      const symbol = valueSymbol(checker, node);
      const declaration = symbol?.valueDeclaration;
      if (declaration && ts.isVariableDeclaration(declaration) && ts.isIdentifier(declaration.name)) {
        if (!(declaration.parent.flags & ts.NodeFlags.Const)) return { why: `${quote(node)} is a \`${declaration.parent.flags & ts.NodeFlags.Let ? 'let' : 'var'}\` binding, which can be reassigned` };
        if (!declaration.initializer) return { why: `${quote(node)} is declared without a value` };
        return { values: [{ node: declaration.initializer }] };
      }
      if (declaration && ts.isParameter(declaration) && ts.isIdentifier(declaration.name)) return this.parameter(declaration, [], node);
      if (declaration && ts.isBindingElement(declaration)) return this.binding(declaration, node);
      return { why: bindingReason(node, symbol, declaration) };
    }
    if (ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node)) {
      const keys = memberKeys(node, known);
      const owners = this.objects(node.expression, known, seen);
      if (owners.array) return { why: `${quote(node)} is an array element, which the build does not follow` };
      if (owners.why) return { why: owners.why, at: owners.at ?? node.expression, built: owners.built };
      if (keys !== BY_KEY) return this.members(owners.values, keys, node, seen);
      // A computed key may read any member, so every one is checked: all of a map of class text,
      // or all of a map of maps that is indexed again.
      const all = this.members(owners.values, undefined, node, seen);
      const indexed = (ts.isElementAccessExpression(node.parent) || ts.isPropertyAccessExpression(node.parent)) && node.parent.expression === node;
      if (!all.why && (indexed || all.values.every(({ node: value }) => ts.isStringLiteralLike(value) || ts.isNumericLiteral(value)))) return all;
      // Otherwise the members its values name, or failing those, the members its type allows.
      const named = texts(checker, node.argumentExpression, known, this.calls);
      return this.members(owners.values, named?.length ? named : typeKeys(checker, node.argumentExpression), node, seen);
    }
    if (ts.isCallExpression(node)) {
      const fn = localFunction(checker, node.expression);
      if (!fn) return { why: `${quote(node)} is the result of a call the build does not follow` };
      if (!fn.body) return { why: `${quote(node)} calls a function declared without a body` };
      return { values: returnExpressions(fn).map(value => ({ node: value, crossed: true })) };
    }
    return { values: [{ node }] };
  }

  /** The written objects (object literals, an element's attributes) `node` can be. */
  objects(node, known, seen = new Set()) {
    node = unwrap(node);
    if (!node) return { values: [] };
    if (seen.has(node)) return { values: [] };
    const outer = seen;
    seen = new Set(seen).add(node);
    if (ts.isObjectLiteralExpression(node) || ts.isJsxAttributes(node)) return { values: [{ node }] };
    if (ts.isArrayLiteralExpression(node)) return { why: `${quote(node)} is an array, not an object the build can read`, at: node, array: true };
    if (empty(this.checker, node)) return { values: [] };
    if (ts.isConditionalExpression(node)) {
      const test = known(node.condition);
      if (test) return this.objects(test.value ? node.whenTrue : node.whenFalse, known, seen);
      return merge([this.objects(node.whenTrue, known, seen), this.objects(node.whenFalse, known, seen)]);
    }
    if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.AmpersandAmpersandToken) return this.objects(node.right, known, seen);
    if (ts.isBinaryExpression(node) && [ts.SyntaxKind.BarBarToken, ts.SyntaxKind.QuestionQuestionToken].includes(node.operatorToken.kind))
      return merge([this.objects(node.left, known, seen), this.objects(node.right, known, seen)]);
    if (ts.isIdentifier(node) || ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node) || ts.isCallExpression(node)) {
      const found = this.values(node, known, outer);
      if (found.why) return found;
      if (found.values.length === 1 && found.values[0].node === node) return { why: `${quote(node)} is not an object the build can read`, at: node };
      return merge(found.values.map(value => this.objects(value.node, value.crossed ? NO_KNOWN : known, seen)));
    }
    return { why: `${quote(node)} is ${describe(node)}, not an object the build can read`, at: node };
  }

  /** The elements an array expression can hold. */
  elements(node, seen = new Set()) {
    node = unwrap(node);
    if (!node || seen.has(node)) return { values: [] };
    const outer = seen;
    seen = new Set(seen).add(node);
    if (ts.isArrayLiteralExpression(node))
      return merge(node.elements.map(element => ts.isSpreadElement(element) ? this.elements(element.expression, seen) : { values: [{ node: element }] }));
    if (ts.isCallExpression(node) && ts.isPropertyAccessExpression(node.expression) && SAME_ELEMENTS.has(node.expression.name.text)) {
      const own = this.elements(node.expression.expression, seen);
      if (node.expression.name.text !== 'concat') return own;
      return merge([own, ...node.arguments.map(argument => this.elements(argument, seen))]);
    }
    if (ts.isIdentifier(node) || ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node)) {
      const found = this.values(node, NO_KNOWN, outer);
      if (found.why) return found;
      if (found.values.length === 1 && found.values[0].node === node) return { why: `${quote(node)} is not an array the build can read`, at: node };
      return merge(found.values.map(value => this.elements(value.node, seen)));
    }
    return { why: `${quote(node)} is not an array the build can read`, at: node };
  }

  /** The `index`th element of the arrays `node` can be. */
  item(node, index, seen = new Set()) {
    node = unwrap(node);
    if (!node || seen.has(node)) return { values: [] };
    const outer = seen;
    seen = new Set(seen).add(node);
    if (ts.isArrayLiteralExpression(node)) {
      if (node.elements.slice(0, index + 1).some(ts.isSpreadElement)) return { why: `${quote(node)} spreads before element ${index}`, at: node };
      return { values: node.elements[index] ? [{ node: node.elements[index] }] : [] };
    }
    if (ts.isIdentifier(node) || ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node) || ts.isCallExpression(node)) {
      const found = this.values(node, NO_KNOWN, outer);
      if (found.why) return found;
      if (found.values.length === 1 && found.values[0].node === node) return { why: `${quote(node)} is not an array the build can read`, at: node };
      return merge(found.values.map(value => this.item(value.node, index, seen)));
    }
    return { why: `${quote(node)} is not an array the build can read`, at: node };
  }

  members(owners, keys, at, seen) {
    const out = [];
    for (const { node: owner, crossed } of owners) {
      const found = this.member(owner, keys, seen);
      if (found.why) return { why: found.why, at, built: found.built };
      out.push(...found.values.map(value => ({ node: value.node, crossed: crossed || value.crossed })));
    }
    return { values: out };
  }

  /** The values `owner` writes for `keys` (undefined: any key); an absent member is no value. */
  member(owner, keys, seen) {
    const out = [];
    const take = key => keys === undefined || keys.includes(key);
    const found = new Set();
    const properties = ts.isJsxAttributes(owner) ? owner.properties : owner.properties;
    for (const property of [...properties].reverse()) {
      if (ts.isSpreadAssignment(property) || ts.isJsxSpreadAttribute(property)) {
        // A spread may write any key: its own members join the earlier ones.
        const inner = this.objects(property.expression, NO_KNOWN, seen);
        if (inner.why) {
          // As on an element, an unread spread writes only the keys its type declares, not an index signature's.
          if (keys && !declares(this.checker, property.expression, keys)) continue;
          return { why: inner.why, built: inner.built };
        }
        const values = this.members(inner.values, keys, property, seen);
        if (values.why) return values;
        out.push(...values.values);
        continue;
      }
      const name = property.name && propertyName(property.name);
      if (name === undefined) {
        if (property.name && ts.isComputedPropertyName(property.name)) return { why: `${quote(property.name)} names a member only at run time` };
        continue;
      }
      if (!take(name) || found.has(name)) continue;
      found.add(name);
      if (ts.isPropertyAssignment(property)) out.push({ node: property.initializer });
      else if (ts.isShorthandPropertyAssignment(property)) out.push({ node: property.name });
      else if (ts.isJsxAttribute(property)) {
        const value = property.initializer;
        if (value && ts.isJsxExpression(value)) { if (value.expression) out.push({ node: value.expression }); }
        else if (value) out.push({ node: value });
      } else return { why: `${quote(property)} is ${describe(property)}, whose value the build does not follow` };
    }
    return { values: out };
  }

  /** A parameter's values at `path`: the matching argument of every use of its function. */
  parameter(parameter, path, at) {
    const key = `${parameter.pos}:${parameter.getSourceFile().fileName}\0${JSON.stringify(path)}`;
    const kept = this.known.get(key);
    if (kept) return replay(kept.reads), kept.result;
    if (this.active.has(key)) {
      this.cyclic = true;
      return { values: [] };
    }
    const outer = this.cyclic;
    this.active.add(key);
    this.cyclic = false;
    const [result, reads] = reading(() => this.arguments(parameter, path, at));
    this.active.delete(key);
    if (!this.cyclic && !result.why) this.known.set(key, { result, reads });
    this.cyclic ||= outer;
    return result;
  }

  arguments(parameter, path, at) {
    const fn = parameter.parent;
    const index = fn.parameters.indexOf(parameter);
    const defaults = parameter.initializer ? [{ node: parameter.initializer }] : [];
    const callback = elementCallback(fn);
    if (callback) {
      if (index !== 0) return { why: `${quote(at)} is the index or array argument of a \`${callback.method}\` callback` };
      const elements = this.elements(callback.array);
      if (elements.why) return { why: `${quote(at)} is an element of ${quote(callback.array)}, which the build cannot read`, at, built: elements.built };
      return this.path(elements.values.map(value => ({ node: value.node, crossed: true })), path, at);
    }
    const uses = this.calls?.uses(fn);
    if (!uses) return { why: `${quote(at)} is a parameter; its value comes from each caller` };
    if (uses.runtime) return { why: `${quote(at)} comes from the page's View, known only at run time`, at, built: true };
    if (uses.escape) return { why: `${quote(at)} is a parameter of a function that is also used as a value (${location(uses.escape)}), so its callers are not known`, at };
    const roots = [...defaults];
    for (const use of uses.sites) {
      if (ts.isJsxOpeningElement(use) || ts.isJsxSelfClosingElement(use)) {
        if (index === 0) roots.push({ node: use.attributes, crossed: true });
        continue;
      }
      const spread = use.arguments.findIndex(ts.isSpreadElement);
      if (spread !== -1 && spread <= index) return { why: `${quote(at)} receives a spread argument at ${location(use)}`, at };
      if (use.arguments[index]) roots.push({ node: use.arguments[index], crossed: true });
    }
    return this.path(roots, path, at);
  }

  /** A destructured binding: its root's values, read along the property path to it. */
  binding(element, at) {
    const path = [];
    let node = element, defaults = [];
    while (ts.isBindingElement(node)) {
      if (node.initializer) defaults.push({ node: node.initializer });
      const pattern = node.parent;
      if (ts.isArrayBindingPattern(pattern)) {
        if (node.dotDotDotToken) return { why: `${quote(at)} is the rest of an array, which the build does not follow` };
        path.unshift({ index: pattern.elements.indexOf(node) });
      } else if (!node.dotDotDotToken) {
        const key = propertyName(node.propertyName ?? node.name);
        if (key === undefined) return { why: `${quote(at)} is destructured under a name known only at run time` };
        path.unshift(key);
      }
      node = pattern.parent;
    }
    let found;
    if (ts.isParameter(node)) found = this.parameter(node, path, at);
    else if (ts.isVariableDeclaration(node) && (node.parent.flags & ts.NodeFlags.Const) && node.initializer) found = this.path([{ node: node.initializer }], path, at);
    else return { why: `${quote(at)} is destructured from a value the build does not follow` };
    return found.why ? found : { values: [...found.values, ...defaults] };
  }

  path(roots, path, at) {
    let values = roots;
    for (const key of path) {
      if (typeof key === 'object') {
        const items = merge(values.map(value => {
          const found = this.item(value.node, key.index);
          return found.why ? found : { values: found.values.map(item => ({ ...item, crossed: item.crossed || value.crossed })) };
        }));
        if (items.why) return { why: items.why, at: items.at ?? at, built: items.built };
        values = items.values;
        continue;
      }
      const owners = merge(values.map(value => {
        const found = this.objects(value.node, NO_KNOWN);
        return found.why ? found : { values: found.values.map(owner => ({ ...owner, crossed: owner.crossed || value.crossed })) };
      }));
      if (owners.why) return { why: owners.why, at: owners.at ?? at, built: owners.built };
      const next = this.members(owners.values, [key], at, new Set());
      if (next.why) return next;
      values = next.values;
    }
    return { values };
  }
}

/** Whether `node`'s type declares any of `keys`; an index signature declares none. */
function declares(checker, node, keys) {
  opaque();
  const type = checker.getTypeAtLocation(node);
  return keys.some(key => type.getProperty(key));
}

function merge(results) {
  const failed = results.find(result => result.why);
  return failed ?? { values: results.flatMap(result => result.values) };
}

/** The member names an access reads: a literal key or a known one; `BY_KEY` when the key is computed. */
function memberKeys(node, known) {
  if (ts.isPropertyAccessExpression(node)) return [node.name.text];
  const argument = node.argumentExpression;
  if (ts.isStringLiteralLike(argument)) return [argument.text];
  const key = known(argument);
  if (key && (typeof key.value === 'string' || typeof key.value === 'number')) return [String(key.value)];
  return BY_KEY;
}
const BY_KEY = Symbol('by key');

/** The literal members a key's type allows; undefined for any member. */
function typeKeys(checker, argument) {
  opaque();
  const type = checker.getTypeAtLocation(argument);
  const parts = type.isUnion() ? type.types : [type];
  return parts.every(part => part.isStringLiteral() || part.isNumberLiteral()) ? parts.map(part => String(part.value)) : undefined;
}

/** The array a callback walks, when it is the element callback of an array method. */
function elementCallback(fn) {
  if (!(ts.isArrowFunction(fn) || ts.isFunctionExpression(fn))) return undefined;
  const call = fn.parent;
  if (!ts.isCallExpression(call) || call.arguments[0] !== fn || !ts.isPropertyAccessExpression(call.expression)) return undefined;
  const method = call.expression.name.text;
  return ELEMENT_CALLBACKS.has(method) ? { array: call.expression.expression, method } : undefined;
}

/** The function a callee names in this program: a declaration or a const bound to one. */
function localFunction(checker, callee) {
  callee = unwrap(callee);
  if (!ts.isIdentifier(callee)) return undefined;
  const declaration = valueSymbol(checker, callee)?.valueDeclaration;
  if (!declaration) return undefined;
  if (ts.isFunctionDeclaration(declaration)) return declaration;
  if (ts.isVariableDeclaration(declaration) && (declaration.parent.flags & ts.NodeFlags.Const)) {
    const value = unwrap(declaration.initializer);
    if (value && (ts.isArrowFunction(value) || ts.isFunctionExpression(value))) return value;
  }
  return undefined;
}

function returnExpressions(fn) {
  if (!ts.isBlock(fn.body)) return [fn.body];
  const out = [];
  const walk = node => {
    if (ts.isReturnStatement(node)) { if (node.expression) out.push(node.expression); return; }
    if (ts.isFunctionLike(node)) return;
    ts.forEachChild(node, walk);
  };
  ts.forEachChild(fn.body, walk);
  return out;
}

/** The binding an identifier reads: through imports, and to the variable a shorthand member names. */
function valueSymbol(checker, node) {
  let symbol = ts.isShorthandPropertyAssignment(node.parent) && node.parent.name === node
    ? checker.getShorthandAssignmentValueSymbol(node.parent) : checker.getSymbolAtLocation(node);
  if (!symbol) opaque();
  // Every hop of an import or re-export is a file the answer depends on.
  for (let hop = symbol, hops = 0; hop && (hop.flags & ts.SymbolFlags.Alias) && hops < 32; hops++) {
    dependOn(hop);
    hop = checker.getImmediateAliasedSymbol(hop);
  }
  if (symbol && (symbol.flags & ts.SymbolFlags.Alias)) symbol = checker.getAliasedSymbol(symbol);
  dependOn(symbol);
  return symbol;
}

function propertyName(name) {
  if (ts.isIdentifier(name) || ts.isStringLiteralLike(name) || ts.isNumericLiteral(name) || ts.isPrivateIdentifier(name)) return name.text;
  if (ts.isComputedPropertyName(name) && (ts.isStringLiteralLike(name.expression) || ts.isNumericLiteral(name.expression))) return name.expression.text;
  return undefined;
}

// What a trace read, so a later build can tell whether its answer still holds: the files whose
// declarations it resolved, the names whose uses it searched, and whether it asked for a type.
let reads = null;
const newReads = () => ({ files: new Set(), names: new Set(), opaque: false });
function dependOn(symbol) {
  if (reads && symbol) for (const declaration of symbol.declarations ?? []) reads.files.add(declaration.getSourceFile());
}
function searched(name) { reads?.names.add(name); }
// A type can come from any file, so an answer that asked for one is not kept.
function opaque() { if (reads) reads.opaque = true; }
function replay(kept) {
  if (!reads) return;
  for (const file of kept.files) reads.files.add(file);
  for (const name of kept.names) reads.names.add(name);
  reads.opaque ||= kept.opaque;
}
/** Runs `work`, returning its result and what it read; the outer reader sees the same reads. */
function reading(work) {
  const outer = reads;
  reads = newReads();
  try {
    const result = work();
    return [result, reads];
  } finally {
    const inner = reads;
    reads = outer;
    replay(inner);
  }
}

/**
 * Every use of each function in `files`, found once per build: elements and calls that use it,
 * through imports, re-exports and namespace members; a use as a plain value is an escape, after
 * which its callers are not known. Type positions are not uses.
 */
function callIndex(files, checker) {
  const indexes = files.map(fileIndex);
  const defaultNames = new Set(indexes.flatMap(index => index.defaultNames));
  const references = name => indexes.flatMap(index => index.byName.get(name) ?? []);
  const naming = new Map();
  const cache = new Map();
  const target = fn => {
    const name = ts.isFunctionDeclaration(fn) ? fn.name
      : ts.isVariableDeclaration(fn.parent) && ts.isIdentifier(fn.parent.name) ? fn.parent.name : undefined;
    return name && valueSymbol(checker, name);
  };
  return {
    files: new Set(files),
    filesNaming(name) {
      let found = naming.get(name);
      if (!found) naming.set(name, found = files.filter((file, at) => indexes[at].byName.has(name)));
      return found;
    },
    uses(fn) {
      const kept = cache.get(fn);
      if (kept) return replay(kept.reads), kept.result;
      const [result, reads] = reading(() => this.search(fn));
      cache.set(fn, { result, reads });
      return result;
    },
    search(fn) {
      const symbol = target(fn);
      let result;
      if (!symbol) result = { sites: [], escape: fn };
      // A page's `render` is called by the View runtime with the page's View.
      else if (symbol.name === 'render' && ts.getCombinedModifierFlags(fn.parent && ts.isVariableDeclaration(fn.parent) ? fn.parent : fn) & ts.ModifierFlags.Export)
        result = { sites: [], escape: fn, runtime: true };
      else {
        const names = new Set([symbol.name]), symbols = new Set([symbol]), sites = [], counted = new Set();
        // A default export is imported under any name.
        const byDefault = () => { for (const name of defaultNames) names.add(name); };
        if (ts.getCombinedModifierFlags(fn.parent && ts.isVariableDeclaration(fn.parent) ? fn.parent : fn) & ts.ModifierFlags.Default) byDefault();
        let escape;
        for (let grew = true; grew;) {
          grew = false;
          for (const name of [...names]) {
            searched(name);
            for (const reference of references(name)) {
              if (counted.has(reference)) continue;
              let resolved;
              try { resolved = valueSymbol(checker, reference); } catch { resolved = undefined; }
              if (!symbols.has(resolved)) continue;
              counted.add(reference);
              const use = classifyUse(reference, checker);
              if (use.alias === 'default' || use.exportDefault) { const before = names.size; byDefault(); grew ||= names.size > before; }
              else if (use.alias && !names.has(use.alias)) { names.add(use.alias); grew = true; }
              if (use.member) {
                const alias = valueSymbol(checker, use.member);
                if (alias && !symbols.has(alias)) { symbols.add(alias); grew = true; }
                if (!names.has(use.member.text)) { names.add(use.member.text); grew = true; }
              }
              if (use.site) sites.push(use.site);
              if (use.escape) escape ??= use.escape;
            }
          }
        }
        result = { sites, escape };
      }
      return result;
    },
  };
}

// A parsed file is reused while its text is unchanged, so what syntax alone says about it is kept with it.
const fileIndexes = new WeakMap();
/** A file's identifiers by name, its default-import names, and its className attributes and spreads. */
function fileIndex(file) {
  let index = fileIndexes.get(file);
  if (index) return index;
  index = { byName: new Map(), defaultNames: [], classNames: [], spreads: [] };
  const collect = node => {
    if (ts.isImportClause(node) && node.name) index.defaultNames.push(node.name.text);
    if (ts.isIdentifier(node)) {
      const list = index.byName.get(node.text);
      if (list) list.push(node); else index.byName.set(node.text, [node]);
    }
    if (ts.isJsxAttribute(node) && node.name.getText(file) === 'className' && node.initializer)
      index.classNames.push({ expression: ts.isJsxExpression(node.initializer) ? node.initializer.expression : node.initializer, tag: elementKind(node.parent.parent), focusable: takesFocus(node.parent, file) });
    if (ts.isJsxSpreadAttribute(node)) {
      const attributes = node.parent.properties;
      const overridden = attributes.slice(attributes.indexOf(node) + 1).some(prop => ts.isJsxAttribute(prop) && prop.name.getText(file) === 'className');
      if (!overridden) index.spreads.push({ node, tag: elementKind(node.parent.parent), at: index.classNames.length });
    }
    ts.forEachChild(node, collect);
  };
  collect(file);
  fileIndexes.set(file, index);
  return index;
}

// An element its props make a keyboard stop: a drag element, or one that hears the pointer and
// reports a value. A spread may carry either.
function takesFocus(attributes, file) {
  const names = new Set(attributes.properties.filter(ts.isJsxAttribute).map(prop => prop.name.getText(file)));
  return names.has('drag') || (names.has('onPointer') && names.has('aria-valuenow')) || attributes.properties.some(ts.isJsxSpreadAttribute);
}

function classifyUse(reference, checker) {
  let node = reference;
  const parent = node.parent;
  if (ts.isFunctionDeclaration(parent) && parent.name === node) return {};
  if (ts.isVariableDeclaration(parent) && parent.name === node) return {};
  if (ts.isImportSpecifier(parent) || ts.isExportSpecifier(parent) || ts.isImportClause(parent) || ts.isNamespaceImport(parent)) return { alias: parent.name?.text };
  if (ts.isExportAssignment(parent)) return { exportDefault: true };
  if (ts.isTypeQueryNode(parent) || ts.isQualifiedName(parent) || ts.isTypeReferenceNode(parent)) return {};
  if (ts.isPropertyAccessExpression(parent) && parent.name === node) node = parent;
  const owner = node.parent;
  if ((ts.isJsxOpeningElement(owner) || ts.isJsxSelfClosingElement(owner)) && owner.tagName === node) return { site: owner };
  if (ts.isJsxClosingElement(owner)) return {};
  if (ts.isCallExpression(owner) && owner.expression === node) return { site: owner };
  // `Part.displayName = …` reads the function without passing it on.
  if (ts.isPropertyAccessExpression(owner) && owner.expression === node) return {};
  if (ts.isTypeOfExpression(owner)) return {};
  if (ts.isBinaryExpression(owner) && owner.left === node && owner.operatorToken.kind === ts.SyntaxKind.EqualsToken) return {};
  // `Parent.Part = Part`: uses of `Parent.Part` are uses of `Part`.
  if (ts.isBinaryExpression(owner) && owner.right === node && owner.operatorToken.kind === ts.SyntaxKind.EqualsToken && ts.isPropertyAccessExpression(owner.left))
    return { member: owner.left.name };
  // Passed where its type cannot be called (`unknown`, a non-function), it is only held, never called.
  if (ts.isCallExpression(owner) && owner.arguments.includes(node)) opaque();
  const contextual = ts.isCallExpression(owner) && owner.arguments.includes(node) ? checker.getContextualType(node) : undefined;
  if (contextual && !(contextual.flags & ts.TypeFlags.Any) && !contextual.getCallSignatures().length && !contextual.getConstructSignatures().length && !contextual.isUnion()) return {};
  return { escape: node };
}

/** `cx` imported from `@lingxia/view`, under any local name. */
function isCx(checker, callee) {
  if (!ts.isIdentifier(callee)) return false;
  const symbol = checker.getSymbolAtLocation(callee);
  dependOn(symbol);
  const declaration = symbol?.declarations?.[0];
  if (!declaration || !ts.isImportSpecifier(declaration)) return false;
  const imported = (declaration.propertyName ?? declaration.name).text;
  const module = declaration.parent.parent.parent.moduleSpecifier;
  return imported === 'cx' && ts.isStringLiteral(module) && module.text === '@lingxia/view';
}

// Reasons are built on paths that often discard them, so each node's text is read once.
const quotes = new WeakMap();
function quote(node) {
  let quoted = quotes.get(node);
  if (quoted === undefined) {
    const text = node.getText(node.getSourceFile()).replace(/\s+/g, ' ');
    quoted = `\`${text.length > 60 ? `${text.slice(0, 57)}...` : text}\``;
    quotes.set(node, quoted);
  }
  return quoted;
}

function describe(node) {
  const words = ts.SyntaxKind[node.kind].replace(/(Expression|Literal|Assignment|Keyword|Declaration)$/, ' $1').replace(/([a-z])([A-Z])/g, '$1 $2').toLowerCase().trim();
  return `${/^[aeiou]/.test(words) ? 'an' : 'a'} ${words}`;
}

function bindingReason(node, symbol, declaration) {
  const name = quote(node);
  if (!symbol) return `${name} cannot be resolved`;
  declaration ??= symbol.declarations?.[0];
  if (!declaration) return `${name} has no declaration the build can read`;
  if (ts.isParameter(declaration)) return `${name} is a parameter; its value comes from each caller`;
  if (ts.isFunctionDeclaration(declaration) || ts.isClassDeclaration(declaration)) return `${name} is a ${ts.isClassDeclaration(declaration) ? 'class' : 'function'}, not class text`;
  if (ts.isVariableDeclaration(declaration)) return `${name} is declared without a value`;
  return `${name} is ${describe(declaration)}, not a constant`;
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

/** Every `className` in `file` as checked usages; what the build cannot follow goes to `untraced`.
 * A usage is `complete` when its whole className was followed to literals. */
function collectClassUsage(file, checker, untraced, calls) {
  const usages = [];
  const record = (expression, tag, focusable) => {
    const result = answer(expression, calls, () => traceClasses(checker, expression, NO_KNOWN, calls));
    const complete = result.untraced.length === 0;
    for (const { text, node } of result.pieces) usages.push({ value: text, tag, focusable, location: location(node), complete });
    untraced.push(...result.untraced);
  };
  const spread = ({ node, tag }) => {
    const values = answer(node, calls, () => {
      const owners = calls.flow.objects(node.expression, NO_KNOWN);
      return owners.why ? owners : calls.flow.members(owners.values, ['className'], node.expression, new Set());
    });
    // An unread spread carries classes only through a declared `className`, not an index signature.
    if (!values.why) for (const value of values.values) record(value.node, tag, true);
    else if (declares(checker, node.expression, ['className']))
      untraced.push({ node: values.at ?? node.expression, reason: `the spread ${quote(node.expression)} may carry a className: ${values.why}`, built: Boolean(values.built) });
  };
  try {
    const { classNames, spreads } = fileIndex(file);
    let next = 0;
    // In source order, as a reader meets them.
    classNames.forEach(({ expression, tag, focusable }, index) => {
      while (next < spreads.length && spreads[next].at <= index) spread(spreads[next++]);
      record(expression, tag, focusable);
    });
    while (next < spreads.length) spread(spreads[next++]);
  } catch (error) {
    untraced.push({ node: file, reason: `the class tracer failed in this file: ${error?.message ?? error}` });
  }
  return usages;
}

// Answers kept across pages and builds, by the className or spread they answer; a parsed file is
// reused while its text is unchanged, so an answer holds while every file it read is still the same.
const answers = new WeakMap();
function answer(root, calls, work) {
  if (!root) return work();
  const kept = answers.get(root);
  if (kept && holds(kept, calls)) return kept.result;
  const [result, read] = reading(work);
  if (!read.opaque) answers.set(root, { result, files: read.files, names: [...read.names].map(name => [name, calls.filesNaming(name)]) });
  return result;
}
function holds(kept, calls) {
  // The default library is one parse shared by every program.
  for (const file of kept.files) if (!calls.files.has(file) && !file.hasNoDefaultLib) return false;
  for (const [name, files] of kept.names) {
    const now = calls.filesNaming(name);
    if (now.length !== files.length || now.some(file => !files.includes(file))) return false;
  }
  return true;
}

/** Class usages across `files`, and each place the build cannot follow a className to literals,
 * as `{ location, expression, reason, built }`, once per place and reason. `built` marks a value
 * assembled at run time (concatenation, a joined template hole, the page's data), whose classes the
 * table may lack; any other place is whole words the tracer could not follow. */
export function collectClassAnalysis(files, checker) {
  const untraced = [];
  const calls = callIndex(files, checker);
  calls.flow = new Flow(checker, calls);
  const usages = files.flatMap(file => collectClassUsage(file, checker, untraced, calls));
  const seen = new Set(), places = [];
  for (const { node, reason, built = false } of untraced) {
    const at = location(node), key = `${at}\0${reason}`;
    if (seen.has(key)) continue;
    seen.add(key);
    places.push({ location: at, expression: node.getText(node.getSourceFile()).replace(/\s+/g, ' '), reason, built });
  }
  return { usages, untraced: places };
}
