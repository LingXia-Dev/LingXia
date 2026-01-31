import fs from "fs";
import os from "os";
import path from "path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_STATIC_DIRS, resolveStaticDirs } from "../static-dirs.js";

describe("resolveStaticDirs", () => {
  let tempDir: string;

  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "lingxia-static-dirs-"));
  });

  afterEach(() => {
    fs.rmSync(tempDir, { recursive: true, force: true });
  });

  it("returns defaults when no config is present", () => {
    expect(resolveStaticDirs(tempDir)).toEqual(DEFAULT_STATIC_DIRS);
  });

  it("reads static directories from lxapp.config.ts", () => {
    fs.writeFileSync(
      path.join(tempDir, "lxapp.config.ts"),
      `export default { staticDirs: ['public', 'assets', 'public', '   ', 123] };`,
    );

    expect(resolveStaticDirs(tempDir)).toEqual(["public", "assets"]);
  });

  it("expands glob patterns to concrete directories", () => {
    fs.mkdirSync(path.join(tempDir, "pages/API/images"), { recursive: true });
    fs.mkdirSync(path.join(tempDir, "pages/home/images"), { recursive: true });
    fs.mkdirSync(path.join(tempDir, "pages/home/icons"), { recursive: true });

    fs.writeFileSync(
      path.join(tempDir, "lxapp.config.ts"),
      `export default { staticDirs: ['pages/*/images'] };`,
    );

    expect(resolveStaticDirs(tempDir)).toEqual([
      "pages/API/images",
      "pages/home/images",
    ]);
  });

  it("ignores lingering lingxia.config.json files", () => {
    fs.writeFileSync(
      path.join(tempDir, "lingxia.config.json"),
      JSON.stringify({ staticDirs: ["json-only"] }),
    );

    expect(resolveStaticDirs(tempDir)).toEqual(DEFAULT_STATIC_DIRS);
  });

  it("returns defaults when config is invalid", () => {
    fs.writeFileSync(
      path.join(tempDir, "lxapp.config.ts"),
      `export default { staticDirs: [''] };`,
    );

    expect(resolveStaticDirs(tempDir)).toEqual(DEFAULT_STATIC_DIRS);
  });
});
