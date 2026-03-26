import { describe, expect, it } from "vitest";
import { TemplateManager, type PageBridgeMethod } from "../template.js";

const tm = new TemplateManager();

describe("TemplateManager.generateBridgeMetadata", () => {
  it("returns empty __names for no functions", () => {
    const out = tm.generateBridgeMetadata([]);
    expect(out).toBe("window.__pageBridge = { __names: [] };");
  });

  it("returns only __names array without function bodies", () => {
    const out = tm.generateBridgeMetadata([
      { name: "greet", mode: "notify" },
      { name: "fetch", mode: "call" },
    ]);
    expect(out).toContain("__names");
    expect(out).toContain('"greet"');
    expect(out).toContain('"fetch"');
    // Should NOT contain function definitions or bridge calls
    expect(out).not.toContain("LingXiaBridge");
    expect(out).not.toContain("function");
  });
});

describe("TemplateManager.generatePageBridgeModule", () => {
  it("returns only __names export for no functions", () => {
    const out = tm.generatePageBridgeModule([]);
    expect(out).toBe("export var __names = [];\n");
  });

  it("generates named exports for each bridge function", () => {
    const out = tm.generatePageBridgeModule([
      { name: "greet", mode: "notify" },
      { name: "compute", mode: "call" },
      { name: "stream", mode: "stream" },
    ]);

    // Each function is a named export
    expect(out).toContain("export function greet(");
    expect(out).toContain("export function compute(");
    expect(out).toContain("export function stream(");

    // Uses rest params (strict mode compatible)
    expect(out).toContain("...args");
    expect(out).not.toContain("arguments");

    // Correct bridge calls per mode
    expect(out).toContain("LingXiaBridge.notify('greet'");
    expect(out).toContain("LingXiaBridge.call('compute'");
    expect(out).toContain("LingXiaBridge.callStream('stream'");

    // Metadata on each function
    expect(out).toContain("greet.__logicFunc = true");
    expect(out).toContain("greet.__bridgeMode = 'notify'");
    expect(out).toContain("compute.__bridgeMode = 'call'");
    expect(out).toContain("stream.__bridgeMode = 'stream'");

    // __names export
    expect(out).toContain('export var __names = ["greet","compute","stream"]');
  });

  it("includes payload filter that strips Event objects", () => {
    const out = tm.generatePageBridgeModule([
      { name: "click", mode: "notify" },
    ]);
    // Must contain _fp helper
    expect(out).toContain("function _fp(");
    expect(out).toContain("instanceof Event");
    expect(out).toContain("stopPropagation");
  });
});

describe("TemplateManager.inferBridgeMethods", () => {
  it("infers notify for void return", () => {
    const methods = tm.inferBridgeMethods({
      fire: { params: [], returnType: undefined },
    });
    expect(methods).toEqual([{ name: "fire", mode: "notify" }]);
  });

  it("infers notify for explicit void/Promise<void>", () => {
    const methods = tm.inferBridgeMethods({
      a: { params: [], returnType: "void" },
      b: { params: [], returnType: "Promise<void>" },
      c: { params: [], returnType: "undefined" },
    });
    expect(methods.map((m) => m.mode)).toEqual(["notify", "notify", "notify"]);
  });

  it("infers call for non-void return", () => {
    const methods = tm.inferBridgeMethods({
      compute: { params: [{ name: "x", type: "number" }], returnType: "number" },
      fetch: { params: [], returnType: "Promise<string>" },
    });
    expect(methods).toEqual([
      { name: "compute", mode: "call" },
      { name: "fetch", mode: "call" },
    ]);
  });

  it("infers stream for generator functions", () => {
    const methods = tm.inferBridgeMethods({
      gen: { params: [], generator: true },
    });
    expect(methods).toEqual([{ name: "gen", mode: "stream" }]);
  });

  it("infers stream for AsyncIterable/AsyncGenerator return types", () => {
    const methods = tm.inferBridgeMethods({
      a: { params: [], returnType: "AsyncIterable<Chunk>" },
      b: { params: [], returnType: "AsyncIterator<Item>" },
      c: { params: [], returnType: "AsyncGenerator<Data, Result>" },
    });
    expect(methods.map((m) => m.mode)).toEqual(["stream", "stream", "stream"]);
  });

  it("throws on multi-parameter methods", () => {
    expect(() =>
      tm.inferBridgeMethods({
        bad: {
          params: [
            { name: "a", type: "string" },
            { name: "b", type: "number" },
          ],
        },
      }),
    ).toThrow("must accept zero or one payload parameter");
  });
});

describe("TemplateManager.generateFunctionBridge", () => {
  it("accepts string[] for backward compat and defaults to notify", () => {
    const out = tm.generateFunctionBridge(["doStuff"]);
    expect(out).toContain("LingXiaBridge.notify('doStuff'");
    expect(out).toContain("__names");
  });

  it("returns empty bridge for no functions", () => {
    const out = tm.generateFunctionBridge([]);
    expect(out).toBe("window.__pageBridge = { __names: [] };");
  });

  it("filters Event objects from arguments at runtime", () => {
    const script = tm.generateFunctionBridge([
      { name: "click", mode: "notify" },
    ]);

    const calls: unknown[] = [];
    const fakeWindow = {
      LingXiaBridge: {
        notify(_name: string, payload: unknown) {
          calls.push(payload);
        },
      },
    } as Record<string, unknown>;

    const install = new Function(
      "window",
      "Event",
      `${script}; return window.__pageBridge.click;`,
    );

    // Create a mock Event class
    class MockEvent {
      stopPropagation() {}
    }

    const click = install(fakeWindow, MockEvent);

    // Event object should be filtered out, leaving undefined payload
    click(new MockEvent());
    expect(calls).toEqual([undefined]);

    // Real payload should pass through
    calls.length = 0;
    click({ id: 1 });
    expect(calls).toEqual([{ id: 1 }]);
  });
});
