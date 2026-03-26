import { describe, expect, it } from "vitest";
import { TemplateManager } from "../template.js";
import { TypeGenerator } from "../type-generator.js";

describe("TemplateManager.generateFunctionBridge", () => {
  it("routes wrappers to the expected bridge API", () => {
    const templateManager = new TemplateManager();
    const script = templateManager.generateFunctionBridge([
      { name: "notifyOnly", mode: "notify" },
      { name: "compute", mode: "call" },
      { name: "streamJob", mode: "stream" },
    ]);

    const calls: Array<{ kind: string; name: string; payload: unknown }> = [];
    const fakeWindow = {
      LingXiaBridge: {
        notify(name: string, payload: unknown) {
          calls.push({ kind: "notify", name, payload });
        },
        call(name: string, payload: unknown) {
          calls.push({ kind: "call", name, payload });
          return "call-result";
        },
        callStream(name: string, payload: unknown) {
          calls.push({ kind: "stream", name, payload });
          return "stream-result";
        },
      },
    } as Record<string, unknown>;

    const install = new Function(
      "window",
      "Event",
      `${script}
return window.__pageBridge;`,
    );
    const actions = install(fakeWindow, class Event {});

    expect(actions.compute({ value: 1 })).toBe("call-result");
    expect(actions.streamJob({ value: 2 })).toBe("stream-result");
    expect(actions.notifyOnly({ value: 3 })).toBeUndefined();
    expect(calls).toEqual([
      { kind: "call", name: "compute", payload: { value: 1 } },
      { kind: "stream", name: "streamJob", payload: { value: 2 } },
      { kind: "notify", name: "notifyOnly", payload: { value: 3 } },
    ]);
  });

  it("rejects multiple payload arguments instead of silently packing arrays", () => {
    const templateManager = new TemplateManager();
    const script = templateManager.generateFunctionBridge([
      { name: "compute", mode: "call" },
    ]);

    const fakeWindow = {
      LingXiaBridge: {
        call() {
          return "unexpected";
        },
      },
    } as Record<string, unknown>;

    const install = new Function(
      "window",
      "Event",
      `${script}; return window.__pageBridge.compute;`,
    );
    const compute = install(fakeWindow, class Event {});

    expect(() => compute("a", "b")).toThrow(
      "Page action 'compute' accepts at most one payload argument",
    );
  });
});

describe("TypeGenerator.generatePageTypes", () => {
  it("rejects multi-parameter page actions instead of silently truncating them", () => {
    const generator = new TypeGenerator("/tmp");
    expect(() =>
      generator.generatePageTypes("pages/example/index.ts", {
        data: {},
        methods: {
          compute: {
            params: [
              { name: "payload", type: "{ value: number }" },
              { name: "internal", type: "string" },
            ],
            returnType: "Promise<number>",
          },
        },
      }),
    ).toThrow(
      "Page action 'compute' must accept zero or one payload parameter; found 2.",
    );
  });

  it("imports StreamHandle and preserves stream result typing", () => {
    const generator = new TypeGenerator("/tmp");
    const output = generator.generatePageTypes("pages/example/index.ts", {
      data: {},
      methods: {
        streamJob: {
          params: [{ name: "payload", type: "{ id: string }" }],
          returnType: "AsyncGenerator<Chunk, FinalResult>",
          generator: true,
        },
      },
    });

    expect(output).toContain(
      'import type { StreamHandle } from "@lingxia/bridge";',
    );
    expect(output).toContain(
      "streamJob(payload: { id: string }): StreamHandle<Chunk, FinalResult>;",
    );
  });
});
