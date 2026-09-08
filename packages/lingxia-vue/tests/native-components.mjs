import assert from "node:assert/strict";
import fs from "node:fs";
import { fileURLToPath } from "node:url";
import { parse, compileScript, registerTS } from "@vue/compiler-sfc";
import ts from "typescript";

registerTS(() => ts);
for (const name of ["LxNativeRoot", "LxNativeView", "LxNativeCover", "LxNativeButton", "LxNativeText", "LxVideo"]) {
  const filename = fileURLToPath(new URL(`../dist/${name}.vue`, import.meta.url));
  const { descriptor } = parse(fs.readFileSync(filename, "utf8"), { filename });
  const compiled = compileScript(descriptor, {
    id: name,
    fs: { fileExists: fs.existsSync, readFile: (path) => fs.readFileSync(path, "utf8") },
  });
  assert.ok(compiled.content.includes("style:"), `${name} must declare its constrained style prop`);
  if (name === "LxVideo") {
    assert.ok(compiled.content.includes('"volumeChange"'));
    assert.ok(compiled.content.includes('"timeUpdate"'));
  }
}
