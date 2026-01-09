/**
 * NativeCallbackRegistry - store callback IDs for native components.
 */

const componentCallbacks: Map<string, string> = new Map();

export function setComponentCallback(componentId: string, callbackId: string): void {
  const id = String(componentId ?? '').trim();
  const cb = String(callbackId ?? '').trim();
  if (!id) return;
  if (!cb) {
    componentCallbacks.delete(id);
    return;
  }
  componentCallbacks.set(id, cb);
}

export function getComponentCallback(componentId: string): string | undefined {
  const id = String(componentId ?? '').trim();
  if (!id) return undefined;
  return componentCallbacks.get(id);
}

export function removeComponentCallback(componentId: string): void {
  const id = String(componentId ?? '').trim();
  if (!id) return;
  componentCallbacks.delete(id);
}
