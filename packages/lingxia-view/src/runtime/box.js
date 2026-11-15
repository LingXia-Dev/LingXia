// `useBox(name)`: the laid-out size of the element that takes the handle as `box`.
import { local, cellOf } from './hooks.js';

const handles = new WeakMap(), written = new WeakMap();

/** The local a `box` prop's handle fills, or undefined when it is not a `useBox` handle. */
export const boxBinding = handle => written.get(handle);

export function useBox(name) {
  if (typeof name !== 'string' || !name.length || name.length > 64) throw Error('UI_PROP: useBox takes a name of 1..64 characters');
  const binding = local(name, null, 'box');
  const { cell } = cellOf(binding);
  let handle = handles.get(cell);
  if (!handle) {
    handle = Object.freeze({ get value() { return binding.value; } });
    handles.set(cell, handle); written.set(handle, binding);
  }
  return handle;
}
