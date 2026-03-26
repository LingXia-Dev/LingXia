import { describe, expect, it } from "vitest";
import { extractPageTypes } from "../page-types.js";

describe("extractPageTypes", () => {
  it("extracts data types from Page config", () => {
    const source = `
      Page({
        data: {
          name: "hello",
          count: 0,
          active: true,
          items: [1, 2],
          meta: null,
        },
      });
    `;
    const result = extractPageTypes(source);
    expect(result.data.name.type).toBe("string");
    expect(result.data.count.type).toBe("number");
    expect(result.data.active.type).toBe("boolean");
    expect(result.data.items.type).toBe("array");
    expect(result.data.meta.type).toBe("null");
  });

  it("extracts method signatures", () => {
    const source = `
      Page({
        data: {},
        greet(name: string) {},
        compute: (x: number): number => x * 2,
      });
    `;
    const result = extractPageTypes(source);
    expect(result.methods.greet).toBeDefined();
    expect(result.methods.greet.params).toHaveLength(1);
    expect(result.methods.greet.params[0].name).toBe("name");
    expect(result.methods.greet.params[0].type).toBe("string");
    expect(result.methods.compute).toBeDefined();
  });

  it("excludes lifecycle hooks from methods", () => {
    const source = `
      Page({
        data: {},
        onLoad() {},
        onShow() {},
        onReady() {},
        onHide() {},
        onUnload() {},
        onPullDownRefresh() {},
        greet() {},
      });
    `;
    const result = extractPageTypes(source);
    expect(Object.keys(result.methods)).toEqual(["greet"]);
  });

  it("excludes private methods starting with _", () => {
    const source = `
      Page({
        data: {},
        _internal() {},
        public_method() {},
      });
    `;
    const result = extractPageTypes(source);
    expect(result.methods._internal).toBeUndefined();
    expect(result.methods.public_method).toBeDefined();
  });

  it("detects async and generator methods", () => {
    const source = `
      Page({
        data: {},
        async fetchData(): Promise<string> { return ""; },
        *generate(): Generator<number> { yield 1; },
      });
    `;
    const result = extractPageTypes(source);
    expect(result.methods.fetchData.async).toBe(true);
    expect(result.methods.generate.generator).toBe(true);
  });

  it("handles TypeScript type assertions", () => {
    const source = `
      Page({
        data: {},
        handler() {},
      } as const);
    `;
    const result = extractPageTypes(source);
    expect(result.methods.handler).toBeDefined();
  });

  it("returns empty result when no Page() call found", () => {
    const source = `const x = 1;`;
    const result = extractPageTypes(source);
    expect(result.data).toEqual({});
    expect(result.methods).toEqual({});
  });

  it("extracts return types for bridge mode inference", () => {
    const source = `
      Page({
        data: {},
        notify(): void {},
        compute(): Promise<number> { return Promise.resolve(1); },
        async *stream(): AsyncGenerator<string> {},
      });
    `;
    const result = extractPageTypes(source);
    // void return type is normalized to undefined by extractReturnType
    expect(result.methods.notify.returnType).toBeUndefined();
    expect(result.methods.compute.returnType).toBe("Promise<number>");
    expect(result.methods.stream.returnType).toBe("AsyncGenerator<string>");
    expect(result.methods.stream.generator).toBe(true);
  });
});
