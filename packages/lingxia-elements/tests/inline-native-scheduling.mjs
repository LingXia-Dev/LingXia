import assert from "node:assert/strict";

const frames = new Map();
let nextFrame = 0;
globalThis.requestAnimationFrame = callback => { frames.set(++nextFrame, callback); return nextFrame; };
globalThis.cancelAnimationFrame = id => frames.delete(id);
function tick() {
  const pending = [...frames.values()];
  frames.clear();
  for (const callback of pending) callback();
}
const observers = [];
globalThis.MutationObserver = class {
  constructor(callback) { this.callback = callback; observers.push(this); }
  observe() {}
  disconnect() {}
};
globalThis.CustomEvent = class extends Event {
  constructor(type, options) { super(type); this.detail = options?.detail; }
};
class Element extends EventTarget {
  attrs = new Map();
  style = {};
  childNodes = [];
  parentElement = null;
  isConnected = true;
  tagName = "LX-NATIVE-ROOT";
  localName = "lx-native-root";
  clientWidth = 100;
  clientHeight = 100;
  ownerDocument = document;
  get attributes() { return [...this.attrs].map(([name, value]) => ({ name, value })); }
  getAttribute(name) { return this.attrs.get(name) ?? null; }
  hasAttribute(name) { return this.attrs.has(name); }
  setAttribute(name, value) { this.attrs.set(name, value); }
  removeAttribute(name) { this.attrs.delete(name); }
  toggleAttribute(name, enabled) { if (enabled) this.setAttribute(name, ""); else this.removeAttribute(name); }
  attachShadow() { return this.shadowRoot = {}; }
  querySelectorAll() { return []; }
  contains(node) { return node === this; }
  getBoundingClientRect() { return { x: 0, y: -window.scrollY, left: 0, top: -window.scrollY, right: 100, bottom: 100 - window.scrollY, width: 100, height: 100 }; }
  getClientRects() { return [this.getBoundingClientRect()]; }
}
globalThis.HTMLElement = globalThis.Element = Element;
globalThis.getComputedStyle = () => ({ opacity: "1", display: "block", visibility: "visible" });
globalThis.document = Object.assign(new EventTarget(), {
  documentElement: {}, defaultView: { getComputedStyle }, querySelectorAll: () => [root],
});
const messages = [];
let host;
const send = message => messages.push(message);
globalThis.window = Object.assign(new EventTarget(), {
  scrollX: 0, scrollY: 0, innerWidth: 500, innerHeight: 500,
  LingXiaBridge: { nativeComponents: { send, register: (_id, handler) => { host = handler; return () => {}; } } },
});
const { LxNativeRootElement } = await import("../dist/inline-native/elements.js");
const root = new LxNativeRootElement();
const count = action => messages.filter(message => message.action === action).length;
root.connectedCallback();
tick();
tick();
const initial = root.lastCompileResult();
assert.equal(initial.ok, true);
const initialGeometry = count("geometry.snapshot");
assert.equal(initialGeometry, 1);

document.dispatchEvent(new Event("scroll"));
tick();
assert.equal(root.lastCompileResult(), initial, "scroll must reuse the compiled tree");
assert.equal(count("geometry.snapshot"), initialGeometry, "identical geometry must not cross the bridge");
window.scrollY = 20;
document.dispatchEvent(new Event("scroll"));
tick();
assert.equal(root.lastCompileResult(), initial);
assert.equal(count("geometry.snapshot"), initialGeometry + 1);

// A coalesced scroll must not downgrade an already queued style/DOM change.
observers[0].callback([{ target: root, attributeName: "style" }]);
document.dispatchEvent(new Event("scroll"));
tick();
assert.notEqual(root.lastCompileResult(), initial);
const afterMutation = root.lastCompileResult();
window.dispatchEvent(new Event("resize"));
tick();
assert.notEqual(root.lastCompileResult(), afterMutation, "resize must re-evaluate media-query styles");

const commits = count("root.commit");
host({ action: "root.resyncRequired" });
tick();
assert.equal(count("root.commit"), commits + 1, "host resync must republish the tree");

window.LingXiaBridge.nativeComponents.send = undefined;
window.scrollY = 30;
document.dispatchEvent(new Event("scroll"));
tick();
const beforeRetry = count("geometry.snapshot");
window.LingXiaBridge.nativeComponents.send = send;
tick();
assert.equal(count("geometry.snapshot"), beforeRetry + 1, "failed geometry must not populate the delivery cache");
root.disconnectedCallback();
assert.equal(frames.size, 0, "disconnect must cancel polling and pending publications");
const beforeReconnect = count("root.commit");
window.LingXiaBridge.nativeComponents.send = undefined;
root.connectedCallback();
tick();
window.LingXiaBridge.nativeComponents.send = send;
tick();
assert.equal(count("root.commit"), beforeReconnect + 1, "a dropped initial commit must be retried in full");
root.disconnectedCallback();
assert.equal(frames.size, 0);
console.log("inline native scheduling ok");

