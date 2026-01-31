import { describe, it, expect, beforeEach, afterEach } from "vitest";
import fs from "fs";
import path from "path";
import os from "os";
import { validateViewFile } from "../view-validator.js";
import type { PageFiles } from "../../../types/index.js";

describe("view-validator", () => {
  let tempDir: string;

  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "view-validator-"));
  });

  afterEach(() => {
    fs.rmSync(tempDir, { recursive: true, force: true });
  });

  const createPageFiles = (content: string, ext = ".tsx"): PageFiles => {
    const viewPath = path.join(tempDir, `index${ext}`);
    fs.writeFileSync(viewPath, content);
    return {
      view: {
        path: viewPath,
        exists: true,
        type: ext === ".vue" ? "vue" : "react",
      },
      logic: { path: "", exists: false },
      config: { path: "", exists: false },
      style: { path: "", exists: false },
    };
  };

  describe("PASS cases", () => {
    it.each([
      ["useLingXia() usage", `const { data } = useLingXia(); return data.x;`],
      ["lx in // comment", `// lx.storage\nreturn null;`],
      ["lx in /* comment */", `/* lx.request */ return null;`],
      ["lx in string", `const s = "lx.storage"; return s;`],
      ["lx in template literal", "const s = `lx.device`; return s;"],
      ["partial match xlx/lxapp", `const xlx = 1; const lxapp = 2;`],
    ])("%s", (_, code) => {
      expect(() => validateViewFile(createPageFiles(code))).not.toThrow();
    });

    it("lx in Vue HTML comment", () => {
      expect(() =>
        validateViewFile(
          createPageFiles(
            `<template><!-- lx.storage --><div/></template>`,
            ".vue",
          ),
        ),
      ).not.toThrow();
    });

    it("non-existent view file", () => {
      const pf: PageFiles = {
        view: { path: "/no/file.tsx", exists: false, type: "react" },
        logic: { path: "", exists: false },
        config: { path: "", exists: false },
        style: { path: "", exists: false },
      };
      expect(() => validateViewFile(pf)).not.toThrow();
    });
  });

  describe("FAIL cases", () => {
    it.each([
      ["lx.storage", `const v = lx.storage.get('k');`],
      ["lx.request", `lx.request({ url: '/' });`],
      ["lx.device", `const i = lx.device.getInfo();`],
    ])("detects %s", (api, code) => {
      expect(() => validateViewFile(createPageFiles(code))).toThrow(
        new RegExp(api),
      );
    });

    it("detects in Vue script", () => {
      expect(() =>
        validateViewFile(
          createPageFiles(`<script setup>lx.storage.get('k')</script>`, ".vue"),
        ),
      ).toThrow(/lx\.storage/);
    });

    it("error message includes guidance", () => {
      try {
        validateViewFile(createPageFiles(`lx.storage.get('k')`));
        expect.fail("Should throw");
      } catch (e: any) {
        expect(e.message).toContain("lx.storage");
        expect(e.message).toContain("useLingXia()");
      }
    });
  });
});
