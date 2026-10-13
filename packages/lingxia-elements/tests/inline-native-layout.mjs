import assert from "node:assert/strict";
import { observeNativeLayout } from "../dist/inline-native/layout.js";

let nextFrame = 0;
const frames = new Map();
globalThis.requestAnimationFrame = callback => { frames.set(++nextFrame, callback); return nextFrame; };
globalThis.cancelAnimationFrame = id => frames.delete(id);
const tick = () => {
  const callbacks = [...frames.values()];
  frames.clear();
  for (const callback of callbacks) callback();
};
let observer;
globalThis.MutationObserver = class {
  constructor(callback) { this.notify = callback; observer = this; }
  observe() {}
  disconnect() { this.disconnected = true; }
};
globalThis.document = { documentElement: {} };
globalThis.getComputedStyle = element => { element.styleReads++; return element.style; };

function element(parentElement = null) {
  return {
    parentElement, isConnected: true, children: [], style: { color: "red", opacity: "1" },
    rect: { x: 0, y: 0, width: 100, height: 100 }, clientWidth: 100, clientHeight: 100,
    rectReads: 0, styleReads: 0, queries: 0,
    querySelectorAll() { this.queries++; return this.children; },
    getBoundingClientRect() { this.rectReads++; return this.rect; },
    contains(node) { return node === this || this.children.includes(node); },
  };
}

const ancestor = element();
const first = element(ancestor);
const second = element(ancestor);
const firstChanges = [];
const secondChanges = [];
const stopFirst = observeNativeLayout(first, change => firstChanges.push(change));
const stopSecond = observeNativeLayout(second, change => secondChanges.push(change));
tick();
assert.deepEqual(firstChanges, ["style"]);
assert.deepEqual(secondChanges, ["style"]);
assert.equal(ancestor.rectReads, 1, "shared ancestors are measured once for all roots");
assert.equal(ancestor.styleReads, 1);
const queries = first.queries + second.queries;
tick();
assert.equal(firstChanges.length, 1, "idle roots do not request compilation");
assert.equal(first.queries + second.queries, queries, "idle sampling reuses DOM membership");

first.rect.y = -10;
tick();
assert.deepEqual(firstChanges, ["style", "geometry"], "scrolling changes geometry without invalidating the tree");
assert.equal(secondChanges.length, 1);

// No MutationObserver event: CSSOM, pseudo-classes and media queries still update paint.
ancestor.style.color = "blue";
tick();
assert.equal(firstChanges.at(-1), "style");
assert.equal(secondChanges.at(-1), "style");

const child = element(first);
first.children.push(child);
observer.notify([{ type: "childList", target: first }]);
tick();
assert.equal(child.rectReads, 1, "a changed subtree invalidates membership");
child.style.opacity = ".5";
tick();
assert.equal(firstChanges.at(-1), "style");

const replacementAncestor = element();
first.parentElement = replacementAncestor;
observer.notify([{ type: "childList", target: ancestor }]);
tick();
assert.equal(replacementAncestor.rectReads, 1, "reparenting refreshes ancestor membership");
replacementAncestor.style.color = "green";
tick();
assert.equal(firstChanges.at(-1), "style");

first.isConnected = false;
const reads = first.rectReads;
tick();
assert.equal(first.rectReads, reads, "detached roots need no measurements");
stopFirst();
stopSecond();
assert.equal(frames.size, 0, "the last disconnect stops the frame sampler");
assert.equal(observer.disconnected, true);
console.log("inline native layout observation ok");
