// Scoped values a component provides to its subtree. Lookup is by the compiler's `__lxid`, so a
// hot-swapped module's readers still match providers built from the old module object.
import { render } from './state.js';
import { brand, isNode } from './nodes.js';
import { noteContext } from './memo.js';
import { cellOf } from './hooks.js';

const unstamped = new WeakMap();
let unnamed = 0;

export function contextId(context) {
  if (typeof context.__lxid === 'string') return context.__lxid;
  let id = unstamped.get(context);
  if (id === undefined) unstamped.set(context, id = `context#${++unnamed}`);
  return id;
}

/** The entry `id` resolves to in a context chain, or undefined for the default. */
export function lookup(chain, id) {
  while (chain && chain.id !== id) chain = chain.next;
  return chain;
}

// Values are compared on recall, so only plain data, functions and handles are allowed.
function plain(value, budget) {
  if (value === null || typeof value !== 'object' || --budget.left < 0 || brand in value || cellOf(value)) return true;
  if (Array.isArray(value)) return value.every(each => plain(each, budget));
  const proto = Object.getPrototypeOf(value);
  if (proto !== Object.prototype && proto !== null) return false;
  // Getters are skipped: reading a handle's state here would count as a render read.
  for (const descriptor of Object.values(Object.getOwnPropertyDescriptors(value)))
    if ('value' in descriptor && !plain(descriptor.value, budget)) return false;
  return true;
}

function checkValue(value) {
  if (!plain(value, { left: 256 })) throw Error('UI_CONTEXT: value must be plain data, functions or handles');
}

export function createContext(...args) {
  if (args.length !== 1) throw Error('UI_CONTEXT: createContext takes a default value');
  if (render.active) throw Error('UI_CONTEXT: createContext belongs at module top level');
  checkValue(args[0]);
  const context = {};
  const Provider = () => { throw Error('UI_CONTEXT: a Provider renders only as an element'); };
  Object.defineProperty(Provider, '__lxContext', { value: context });
  Object.defineProperty(Provider, '__lxid', { get: () => `${contextId(context)}.Provider` });
  Object.defineProperty(context, 'Provider', { value: Provider, enumerable: true });
  Object.defineProperty(context, 'defaultValue', { value: args[0] });
  return context;
}

export function isProvider(tag) { return Boolean(tag.__lxContext); }

/** Resolves a Provider's one child with the provided value visible to it. */
export function provide(tag, props, children, chain, resolve) {
  for (const key in props) if (key !== 'value' && key !== 'key') throw Error(`UI_CONTEXT: a Provider takes only value, not ${key}`);
  if (!('value' in props)) throw Error('UI_CONTEXT: a Provider needs a value');
  checkValue(props.value);
  const child = children.length === 1 ? children[0] : undefined;
  if (children.length > 1 || (child != null && child !== false && !isNode(child))) throw Error('UI_CONTEXT: a Provider wraps one element');
  const result = resolve(child, { id: contextId(tag.__lxContext), value: props.value, next: chain });
  // A fragment is placed later by an enclosing element, out of this provider's reach.
  if (result?.fragment) throw Error('UI_CONTEXT: a Provider wraps one element');
  return result;
}

export function useContext(context) {
  if (!render.active || render.scope === null) throw Error('UI_SCOPE: useContext outside render');
  if (context?.Provider?.__lxContext !== context) throw Error('UI_CONTEXT: expected a context from createContext');
  const id = contextId(context), entry = lookup(render.contexts, id);
  const value = entry ? entry.value : context.defaultValue;
  noteContext(id, context, value);
  return value;
}
