import assert from "node:assert/strict";
import { test } from "node:test";
import { packageText } from "../package-text.js";

// As `lingxia gen i18n` compiles a package's words: one function per key, per language.
const words = {
  source: "en",
  languages: {
    en: { "@jinshu.agent.edit": ({ title }) => ["Edit ", title], "@jinshu.agent.untitled": () => "Untitled" },
    zh: { "@jinshu.agent.edit": ({ title }) => `编辑${title}` },
  },
  formatter: () => ({}),
};

test("packageText reads the package's language, then its source, and refuses an unknown key", () => {
  assert.equal(packageText(words, "@jinshu.agent.edit", { title: "W3" }, "zh-CN"), "编辑W3");
  assert.equal(packageText(words, "@jinshu.agent.edit", { title: "W3" }, "en-GB"), "Edit W3");
  assert.equal(packageText(words, "@jinshu.agent.untitled", undefined, "zh-CN"), "Untitled");
  assert.equal(packageText(words, "@jinshu.agent.edit", { title: "W3" }, "ja-JP"), "Edit W3");
  assert.throws(() => packageText(words, "@jinshu.nope", undefined, "en"), /no package word '@jinshu.nope'/);
  assert.throws(() => packageText(words, "@jinshu.agent.untitled", undefined, ""), /pass the caller's language/);
});

test("the lxapp's override in that language wins, and a miss falls back to the package", () => {
  const asked = [];
  globalThis.__lingxiaAppText = (key, language, args) => {
    asked.push([key, language, JSON.parse(args)]);
    return language === "zh-CN" && key === "@jinshu.agent.edit" ? "修改《W3》" : null;
  };
  try {
    assert.equal(packageText(words, "@jinshu.agent.edit", { title: "W3" }, "zh-CN"), "修改《W3》");
    assert.equal(packageText(words, "@jinshu.agent.edit", { title: "W3" }, "en-US"), "Edit W3");
    assert.deepEqual(asked, [["@jinshu.agent.edit", "zh-CN", { title: "W3" }], ["@jinshu.agent.edit", "en-US", { title: "W3" }]]);
  } finally {
    delete globalThis.__lingxiaAppText;
  }
});
