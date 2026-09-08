import assert from "node:assert/strict";
import { applyNativeAriaAttributes } from "../dist/text_component_shared.js";

const attrs = {};
const el = {
  setAttribute(name, value) {
    attrs[name] = String(value);
  },
  removeAttribute(name) {
    delete attrs[name];
  },
};

applyNativeAriaAttributes(el, {
  "aria-label": "More native menu actions",
  automationId: "video-native-menu-more",
});
assert.equal(attrs["aria-label"], "More native menu actions");
assert.equal(attrs["automation-id"], "video-native-menu-more");

applyNativeAriaAttributes(el, { ariaLabel: "Close native menu" });
assert.equal(attrs["aria-label"], "Close native menu");
assert.equal(attrs["automation-id"], undefined);

console.log("native-aria-attributes: ok");
