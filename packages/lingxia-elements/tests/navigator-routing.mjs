import assert from "node:assert/strict";

class FakeHTMLElement {
  attrs = new Map();
  events = [];
  getAttribute(name) {
    return this.attrs.has(name) ? this.attrs.get(name) : null;
  }
  dispatchEvent(event) {
    this.events.push(event);
    return true;
  }
}

const calls = [];
globalThis.HTMLElement = FakeHTMLElement;
globalThis.CustomEvent = class {
  constructor(type, init) {
    this.type = type;
    this.detail = init?.detail;
  }
};
globalThis.window = {
  LingXiaBridge: {
    invoke: async (route, params) => calls.push(["host", route, params]),
    raw: { call: async (method, params) => calls.push(["logic", method, params]) },
  },
};

const { LxNavigatorElement } = await import("../dist/navigator.js");

async function tap(attrs) {
  calls.length = 0;
  const el = new LxNavigatorElement();
  for (const [key, value] of Object.entries(attrs)) el.attrs.set(key, value);
  el.handleClick({ preventDefault() {} });
  await new Promise((resolve) => setTimeout(resolve, 0));
  const outcome = el.events.find((event) => event.type === "success" || event.type === "fail");
  return { calls: [...calls], outcome };
}

// A url defaults to the system browser.
let result = await tap({ url: "https://example.com" });
assert.deepEqual(result.calls, [["host", "device.openUrl", { url: "https://example.com", target: "external" }]]);

// In-app placements go through Logic's lx.surface.
result = await tap({ url: "https://example.com", as: "aside", edge: "left", size: '{"width":"40%"}' });
assert.deepEqual(result.calls, [
  ["logic", "surface.openUrl", { url: "https://example.com", options: { as: "aside", edge: "left", size: { width: "40%" } } }],
]);

result = await tap({ url: "https://example.com", as: "tab" });
assert.deepEqual(result.calls, [["logic", "surface.openUrl", { url: "https://example.com", options: { as: "tab" } }]]);

result = await tap({
  page: "detail",
  query: '{"id":1}',
  as: "float",
  position: "bottom",
  interaction: '{"closeButton":true}',
});
assert.deepEqual(result.calls, [
  [
    "logic",
    "surface.openPage",
    { page: "detail", options: { as: "float", query: { id: 1 }, position: "bottom", interaction: { closeButton: true } } },
  ],
]);

// Without `as`, a page follows open-type.
result = await tap({ page: "detail", "open-type": "redirect" });
assert.deepEqual(result.calls, [["host", "navigation.redirectTo", { page: "detail" }]]);

// app-id infers the lxapp target; a placement is refused there.
result = await tap({ "app-id": "other", page: "home" });
assert.deepEqual(result.calls, [["host", "navigator.navigateToApp", { appId: "other", page: "home" }]]);
result = await tap({ "app-id": "other", as: "float" });
assert.equal(result.calls.length, 0);
assert.equal(result.outcome.type, "fail");

// Mismatched placements fail before reaching the bridge.
result = await tap({ url: "https://example.com", as: "float" });
assert.equal(result.calls.length, 0);
assert.match(result.outcome.detail.errMsg, /external, tab, or aside/);
result = await tap({ page: "detail", as: "aside" });
assert.equal(result.calls.length, 0);
assert.match(result.outcome.detail.errMsg, /float or window/);

console.log("navigator routing ok");
