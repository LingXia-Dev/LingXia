import fs from "fs";
import os from "os";
import path from "path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  defineConfig,
  extractPluginSpecs,
  loadLxappConfig,
  loadLxpluginConfig,
} from "../lxapp-config.js";

describe("build config loader", () => {
  let tempDir: string;

  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "build-config-"));
  });

  afterEach(() => {
    fs.rmSync(tempDir, { recursive: true, force: true });
  });

  it("returns undefined when no config file exists", () => {
    expect(loadLxappConfig(tempDir)).toBeUndefined();
    expect(loadLxpluginConfig(tempDir)).toBeUndefined();
  });

  it("loads lxapp config from lxapp.config.ts", () => {
    fs.writeFileSync(
      path.join(tempDir, "lxapp.config.ts"),
      `export default { staticDirs: ['media'] };`,
    );

    expect(loadLxappConfig(tempDir)).toEqual({ staticDirs: ["media"] });
  });

  it("loads lxplugin config from lxplugin.config.ts", () => {
    fs.writeFileSync(
      path.join(tempDir, "lxplugin.config.ts"),
      `export default { alias: { '@utils': 'utils' } };`,
    );

    expect(loadLxpluginConfig(tempDir)).toEqual({
      alias: { "@utils": "utils" },
    });
  });

  it("ignores invalid exports", () => {
    fs.writeFileSync(
      path.join(tempDir, "lxapp.config.ts"),
      `export default 42;`,
    );

    expect(loadLxappConfig(tempDir)).toBeUndefined();
  });

  it("defineConfig returns the provided config", () => {
    const config = { staticDirs: ["foo"] };
    expect(defineConfig(config)).toBe(config);
  });

  it("normalizes plugin specs from config object", () => {
    const config = defineConfig({
      plugins: {
        react: [
          "vite-plugin-one",
          {
            module: "./plugins/local-plugin.ts",
            namedExport: "createPlugin",
            options: { foo: true },
          },
          { module: "" },
        ],
      },
    });

    const specs = extractPluginSpecs(config);
    expect(specs?.react).toEqual([
      { module: "vite-plugin-one" },
      {
        module: "./plugins/local-plugin.ts",
        namedExport: "createPlugin",
        options: { foo: true },
      },
    ]);
    expect(specs?.vue).toBeUndefined();
  });

  it("applies shared plugin array to all frameworks", () => {
    const sharedPlugin = { name: "shared-plugin", transform() {} };
    const config = defineConfig({
      plugins: ["vite-plugin-shared", sharedPlugin],
    });

    const specs = extractPluginSpecs(config);
    expect(specs?.react).toEqual([
      { module: "vite-plugin-shared" },
      { plugin: sharedPlugin },
    ]);
    expect(specs?.vue).toEqual([
      { module: "vite-plugin-shared" },
      { plugin: sharedPlugin },
    ]);
  });
});
