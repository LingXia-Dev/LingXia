import { describe, expect, it } from "vitest";
import {
  transformAppRegistration,
  transformPageRegistration,
} from "../logic-registration-transformer.js";

const PAGE_PATH = "pages/demo/index.ts";

describe("transformPageRegistration", () => {
  it("rewrites Page() into explicit registration with binding metadata", () => {
    const source = `
      Page({
        data: {},
        onLoad() {},
        greet() {},
        _private() {}
      });
    `;

    const output = transformPageRegistration({
      logicContent: source,
      pagePath: PAGE_PATH,
    });

    expect(output).toContain(`globalThis.__registerPage("${PAGE_PATH}"`);
    expect(output).toContain('"{\\"handlers\\":[\\"greet\\"]}"');
  });

  it("only includes function handlers in page metadata", () => {
    const source = `
      Page({
        data: {},
        cache: null,
        counter: 1,
        onLoad() {},
        helper: () => {},
        label: "demo"
      });
    `;

    const output = transformPageRegistration({
      logicContent: source,
      pagePath: PAGE_PATH,
    });

    expect(output).toContain(
      '"{\\"handlers\\":[\\"helper\\"]}"',
    );
    expect(output).not.toContain('\\"cache\\"');
    expect(output).not.toContain('\\"counter\\"');
    expect(output).not.toContain('\\"label\\"');
    expect(output).not.toContain('\\"onLoad\\"');
  });

  it("keeps referenced page handlers in metadata", () => {
    const source = `
      const handlers = {
        greet: () => {}
      };
      const onLoad = () => {};

      Page({
        data: {},
        onLoad,
        greet: handlers.greet,
        count: sharedCount
      });
    `;

    const output = transformPageRegistration({
      logicContent: source,
      pagePath: PAGE_PATH,
    });

    expect(output).toContain(
      '"{\\"handlers\\":[\\"greet\\"]}"',
    );
    expect(output).not.toContain('\\"count\\"');
    expect(output).not.toContain('\\"onLoad\\"');
  });

  it("drops legacy Page() path arguments", () => {
    const source = `
      Page({
        onLoad() {}
      }, "pages/legacy/index.ts");
    `;

    const output = transformPageRegistration({
      logicContent: source,
      pagePath: PAGE_PATH,
    });

    expect(output).toContain(`globalThis.__registerPage("${PAGE_PATH}"`);
    expect(output).not.toContain("pages/legacy/index.ts");
  });

  it("prefixes plugin page paths", () => {
    const source = `
      Page({
        onShow() {}
      });
    `;

    const output = transformPageRegistration({
      logicContent: source,
      pagePath: PAGE_PATH,
      pluginId: "demo",
    });

    expect(output).toContain('globalThis.__registerPage("plugin/demo/pages/demo/index.ts"');
  });

  it("keeps non-literal Page() configs and falls back to empty metadata", () => {
    const source = `
      const config = createPage();
      Page(config);
    `;

    const output = transformPageRegistration({
      logicContent: source,
      pagePath: PAGE_PATH,
    });

    expect(output).toContain(`globalThis.__registerPage("${PAGE_PATH}", config,`);
    expect(output).toContain('"{\\"handlers\\":[]}"');
  });

  it("throws when Page() is missing", () => {
    expect(() =>
      transformPageRegistration({
        logicContent: "console.log('no page');",
        pagePath: PAGE_PATH,
      }),
    ).toThrow(/No Page\(\) registration found/);
  });
});

describe("transformAppRegistration", () => {
  it("rewrites App() into explicit registration with lifecycle metadata", () => {
    const source = `
      App({
        onLaunch() {},
        onShow() {},
        helper() {}
      });
    `;

    const output = transformAppRegistration({ logicContent: source });

    expect(output).toContain("globalThis.__registerApp({");
    expect(output).toContain('"[\\"onLaunch\\",\\"onShow\\"]"');
    expect(output).toContain("helper() {}");
  });

  it("leaves files without App() unchanged", () => {
    const source = "export const value = 1;";
    expect(transformAppRegistration({ logicContent: source })).toBe(source);
  });

  it("keeps referenced lifecycle handlers in app metadata", () => {
    const source = `
      const lifecycle = {
        onShow: () => {}
      };
      const onLaunch = async () => {};

      App({
        onLaunch,
        onShow: lifecycle.onShow,
        helper: sharedHelper
      });
    `;

    const output = transformAppRegistration({ logicContent: source });

    expect(output).toContain('"[\\"onLaunch\\",\\"onShow\\"]"');
    expect(output).not.toContain('\\"helper\\"');
  });

  it("keeps non-literal App() configs and falls back to empty lifecycle metadata", () => {
    const source = `
      const config = createApp();
      App(config);
    `;

    const output = transformAppRegistration({ logicContent: source });

    expect(output).toContain('globalThis.__registerApp(config, "[]")');
  });
});
