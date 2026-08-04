import assert from "node:assert/strict";
import { shouldApplyPageChromeRevision } from "../dist/page-chrome.js";

assert.equal(shouldApplyPageChromeRevision(4, 3), false);
assert.equal(shouldApplyPageChromeRevision(4, 4), true);
assert.equal(shouldApplyPageChromeRevision(4, 5), true);
