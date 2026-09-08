import assert from "node:assert/strict";
import { collectAuthorTreeFromElement, compileInlineNativeRoot } from "../dist/inline-native/structure.js";

const style = {
  boxShadow: "rgb(0, 0, 0) 0px 2px 4px 0px", transform: "matrix(1, 0, 0, 1, 10, 0)",
  borderTopLeftRadius: "8px", borderTopRightRadius: "0px",
  borderBottomRightRadius: "0px", borderBottomLeftRadius: "0px",
};
const element = {
  tagName: "LX-NATIVE-VIEW", attributes: [], childNodes: [], parentElement: null,
  getAttribute: (name) => name === "id" ? "menu" : null,
  ownerDocument: { defaultView: { getComputedStyle: () => new Proxy(style, { get: (target, key) => target[key] ?? "" }) } },
};
const author = collectAuthorTreeFromElement(element);
const result = compileInlineNativeRoot({ type: "LxNativeRoot", children: [author] });
assert.equal(result.ok, true, "unsupported paint should remain diagnosable without hiding the Root");
assert.equal(result.diagnostics.length, 3);
assert.ok(result.diagnostics.every((diagnostic) => diagnostic.recoverable && diagnostic.message.includes("LxNativeView#menu")));
assert.ok(result.diagnostics.some((diagnostic) => diagnostic.code === "NATIVE_ROOT_UNSUPPORTED_LAYOUT"));
assert.ok(result.diagnostics.some((diagnostic) => diagnostic.message.includes("borderRadius")));
assert.equal(result.root.children[0].props.styleIssues, undefined, "diagnostics must not leak into host props");

Object.assign(style, {
  boxShadow: "none", transform: "none", borderTopRightRadius: "8px",
  borderBottomRightRadius: "8px", borderBottomLeftRadius: "8px",
  transitionDuration: "0s, 0s", backgroundColor: "rgb(10, 20, 30)",
});
const supported = compileInlineNativeRoot({ type: "LxNativeRoot", children: [collectAuthorTreeFromElement(element)] });
assert.deepEqual(supported.diagnostics, []);
assert.equal(supported.root.children[0].props.nativeStyle.backgroundColor, "rgb(10, 20, 30)");
assert.equal(supported.root.children[0].props.nativeStyle.borderRadius, "8px");

for (const corner of ["TopLeft", "TopRight", "BottomRight", "BottomLeft"]) style[`border${corner}Radius`] = "0px";
assert.equal(collectAuthorTreeFromElement(element).props.nativeStyle.borderRadius, "0px",
  "zero radius must override the native button recipe's rounded default");

element.tagName = "LX-VIDEO";
element.baseURI = "https://example.com/page/";
element.getAttribute = (name) => name === "poster" ? "../poster.png" : null;
style.backgroundImage = 'url("https://example.com/poster.png")';
const poster = compileInlineNativeRoot({ type: "LxNativeRoot", children: [collectAuthorTreeFromElement(element)] });
assert.deepEqual(poster.diagnostics, [], "the element's own poster placeholder is supported natively");
style.backgroundImage = "linear-gradient(black, white)";
const gradient = compileInlineNativeRoot({ type: "LxNativeRoot", children: [collectAuthorTreeFromElement(element)] });
assert.ok(gradient.diagnostics.some((diagnostic) => diagnostic.message.includes("backgroundImage")));
