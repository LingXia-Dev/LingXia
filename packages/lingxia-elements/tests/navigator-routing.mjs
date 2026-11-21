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

async function tap(to, attrs = {}) {
  calls.length = 0;
  const el = new LxNavigatorElement();
  el.attrs.set("to", JSON.stringify(to));
  for (const [key, value] of Object.entries(attrs)) el.attrs.set(key, value);
  el.handleClick({ preventDefault() {} });
  await new Promise((resolve) => setTimeout(resolve, 0));
  const outcome = el.events.find((event) => event.type === "success" || event.type === "fail");
  return { calls: [...calls], outcome };
}

// A page pushes; `replace` and `reset` pick the other stack operations.
let result = await tap({ page: "detail", params: { id: 1 } });
assert.deepEqual(result.calls, [["host", "navigation.navigateTo", { page: "detail", query: { id: 1 } }]]);
result = await tap({ page: "detail" }, { replace: "" });
assert.deepEqual(result.calls, [["host", "navigation.redirectTo", { page: "detail" }]]);
result = await tap({ tab: "home" }, { reset: "" });
assert.deepEqual(result.calls, [["host", "navigation.reLaunch", { page: "home" }]]);
result = await tap({ tab: "home" });
assert.deepEqual(result.calls, [["host", "navigation.switchTab", { page: "home" }]]);
result = await tap({ back: 2 });
assert.deepEqual(result.calls, [["host", "navigation.navigateBack", { delta: 2 }]]);

// A url opens in the in-app browser unless `as="external"`.
result = await tap({ url: "https://example.com" });
assert.deepEqual(result.calls, [["logic", "surface.openUrl", { url: "https://example.com", options: {} }]]);
result = await tap({ url: "https://example.com" }, { as: "external" });
assert.deepEqual(result.calls, [["host", "device.openUrl", { url: "https://example.com", target: "external" }]]);
result = await tap({ url: "https://example.com" }, { as: "aside", edge: "left", size: '{"width":"40%"}' });
assert.deepEqual(result.calls, [
  ["logic", "surface.openUrl", { url: "https://example.com", options: { as: "aside", edge: "left", size: { width: "40%" } } }],
]);
result = await tap({ url: "tel:10086" });
assert.deepEqual(result.calls, [["host", "device.makePhoneCall", { phoneNumber: "10086" }]]);

result = await tap({ page: "detail", params: { id: 1 } }, {
  as: "float",
  position: "bottom",
  interaction: '{"closeButton":true}',
});
assert.deepEqual(result.calls, [
  [
    "logic",
    "surface.openPage",
    { page: "detail", options: { as: "float", params: { id: 1 }, position: "bottom", interaction: { closeButton: true } } },
  ],
]);

result = await tap({ app: "other", page: "home", params: { id: "7" }, channel: "draft" });
assert.deepEqual(result.calls, [
  ["host", "navigator.navigateToApp", { appId: "other", page: "home", query: { id: "7" }, channel: "draft" }],
]);

// Shapes that do not name one target, or combine options that do not apply,
// fail before reaching the bridge.
for (const [to, attrs, message] of [
  [{ page: "a", tab: "b" }, {}, /exactly one/],
  [{ app: "other" }, { as: "float" }, /as applies/],
  [{ app: "other", page: "home" }, { as: "float" }, /as applies/],
  [{ app: "other", page: "home" }, { replace: "" }, /replace applies/],
  [{ tab: "home" }, { replace: "" }, /replace applies/],
  [{ url: "https://example.com" }, { as: "float" }, /external, main, or aside/],
  [{ page: "detail" }, { as: "aside" }, /float or window/],
  [{ back: 0 }, {}, /positive integer/],
]) {
  result = await tap(to, attrs);
  assert.equal(result.calls.length, 0, JSON.stringify(to));
  assert.match(result.outcome.detail.errMsg, message);
}

console.log("navigator routing ok");
