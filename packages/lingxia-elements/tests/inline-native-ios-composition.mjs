import assert from "node:assert/strict";
import { IOSRootComposition, iosAnchorExtent } from "../dist/inline-native/ios-composition.js";

const signatures = new Set(Array.from({ length: 4096 }, (_, index) => JSON.stringify(iosAnchorExtent(index))));
assert.equal(signatures.size, 4096, "coincident roots need distinct layer signatures");
for (const index of [-1, .5, 4096, NaN]) assert.throws(() => iosAnchorExtent(index), RangeError);

let ios = true;
globalThis.window = { LingXiaBridge: { platform: { isIOS: () => ios } }, scrollX: 5, scrollY: 80, visualViewport: { scale: 1 } };
globalThis.getComputedStyle = node => ({ opacity: "1", ...node.css });
const rect = (x, y, width, height) => ({ x, y, width, height, left: x, top: y, right: x + width, bottom: y + height });
const box = rect(20, 40, 200, 100);
const elements = [];
globalThis.document = {
  defaultView: { getComputedStyle },
  createElement: () => {
    const element = { style: {}, children: [], setAttribute() {}, append(child) { this.children.push(child); },
      getBoundingClientRect: () => box, remove() { this.removed = true; } };
    elements.push(element);
    return element;
  },
};
const shadow = { children: [], append(child) { this.children.push(child); } };
const parent = { ownerDocument: document, parentElement: null, css: {} };
const child = { tagName: "LX-VIDEO", hasAttribute: () => false, closest: () => root, getBoundingClientRect: () => box };
const root = { ownerDocument: document, parentElement: parent, css: {}, querySelectorAll: () => [child] };
const composition = new IOSRootComposition();
composition.setup(shadow);
composition.setup(shadow);
assert.equal(shadow.children.length, 1, "repeated setup must not add competing layers");
const anchor = shadow.children[0];
composition.setPresentation(false);
assert.equal(anchor.style.pointerEvents, "none");
assert.deepEqual(composition.measure(root).contentRect, { x: 25, y: 120, width: 200, height: 100 });
composition.setPresentation(true);
assert.equal(anchor.style.pointerEvents, "none", "layer discovery before lease readiness must not cover fallback DOM");
composition.setReady(true);
assert.equal(anchor.style.pointerEvents, "auto");
composition.setReady(false);
assert.equal(anchor.style.pointerEvents, "none", "lease fallback immediately releases the placeholder's touches");
composition.setPresentation(true);
assert.equal(anchor.style.pointerEvents, "none", "late presentation must not reactivate fallback input");
for (const css of [{ transform: "matrix(1,0,0,1,1,1)" }, { translate: "1px" }, { zoom: "2" }, { opacity: ".5" }]) {
  parent.css = css;
  assert.equal(composition.measure(root), undefined, "unsupported ancestor geometry must use the overlay");
}
parent.css = {};
window.visualViewport.scale = 2;
assert.equal(composition.measure(root), undefined);
window.visualViewport.scale = 1;
child.getBoundingClientRect = () => rect(0, 40, 220, 100);
assert.equal(composition.measure(root), undefined, "do not clip overflowing native content to the anchor");
child.hasAttribute = () => true;
assert.ok(composition.measure(root), "fallback DOM must not constrain native composition");
composition.destroy();
assert.equal(anchor.removed, true);
assert.equal(composition.measure(root), undefined);
composition.setup(shadow);
assert.notDeepEqual(composition.measure(root).scrollExtent, iosAnchorExtent(0), "reconnected roots must not reclaim a stale layer");
composition.destroy();
ios = false;
const count = elements.length;
new IOSRootComposition().setup(shadow);
assert.equal(elements.length, count, "other platforms must not create iOS scroll anchors");
