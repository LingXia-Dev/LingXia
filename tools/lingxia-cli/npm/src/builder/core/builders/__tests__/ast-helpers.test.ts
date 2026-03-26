import { describe, expect, it } from "vitest";
import { parse } from "@babel/parser";
import {
  type BabelNode,
  AST_PARSE_OPTIONS,
  PAGE_LIFECYCLE_NAMES,
  traverseAst,
  isPageCall,
  unwrapExpression,
  getPropertyName,
} from "../ast-helpers.js";

describe("PAGE_LIFECYCLE_NAMES", () => {
  it("contains only implemented lifecycle hooks", () => {
    const expected = new Set([
      "onLoad",
      "onShow",
      "onReady",
      "onHide",
      "onUnload",
      "onPullDownRefresh",
    ]);
    expect(PAGE_LIFECYCLE_NAMES).toEqual(expected);
  });

  it("does not contain unimplemented hooks", () => {
    expect(PAGE_LIFECYCLE_NAMES.has("onReachBottom")).toBe(false);
    expect(PAGE_LIFECYCLE_NAMES.has("onPageScroll")).toBe(false);
    expect(PAGE_LIFECYCLE_NAMES.has("onShareAppMessage")).toBe(false);
    expect(PAGE_LIFECYCLE_NAMES.has("onResize")).toBe(false);
    expect(PAGE_LIFECYCLE_NAMES.has("onTabItemTap")).toBe(false);
  });
});

describe("traverseAst", () => {
  it("visits all nodes in a simple AST", () => {
    const ast = parse("const x = 1;", AST_PARSE_OPTIONS);
    const types: string[] = [];
    traverseAst(ast.program as BabelNode, (node) => {
      if (node.type) types.push(node.type);
    });
    expect(types).toContain("VariableDeclaration");
    expect(types).toContain("NumericLiteral");
  });

  it("handles null/undefined gracefully", () => {
    const types: string[] = [];
    traverseAst(null, (node) => types.push(node.type!));
    traverseAst(undefined, (node) => types.push(node.type!));
    expect(types).toEqual([]);
  });
});

describe("isPageCall", () => {
  it("matches Page() call expressions", () => {
    const ast = parse("Page({});", AST_PARSE_OPTIONS);
    const calls: BabelNode[] = [];
    traverseAst(ast.program as BabelNode, (node) => {
      if (isPageCall(node)) calls.push(node);
    });
    expect(calls).toHaveLength(1);
  });

  it("ignores non-Page calls", () => {
    const ast = parse("App({}); Component({});", AST_PARSE_OPTIONS);
    const calls: BabelNode[] = [];
    traverseAst(ast.program as BabelNode, (node) => {
      if (isPageCall(node)) calls.push(node);
    });
    expect(calls).toHaveLength(0);
  });
});

describe("unwrapExpression", () => {
  it("unwraps TSAsExpression", () => {
    const node: BabelNode = {
      type: "TSAsExpression",
      expression: { type: "ObjectExpression", properties: [] },
    };
    expect(unwrapExpression(node)?.type).toBe("ObjectExpression");
  });

  it("unwraps ParenthesizedExpression", () => {
    const node: BabelNode = {
      type: "ParenthesizedExpression",
      expression: { type: "Identifier", name: "x" },
    };
    expect(unwrapExpression(node)?.type).toBe("Identifier");
  });

  it("returns null for SpreadElement", () => {
    const node: BabelNode = { type: "SpreadElement" };
    expect(unwrapExpression(node)).toBeNull();
  });

  it("returns null for null/undefined input", () => {
    expect(unwrapExpression(null)).toBeNull();
    expect(unwrapExpression(undefined)).toBeNull();
  });

  it("unwraps nested wrappers", () => {
    const node: BabelNode = {
      type: "TSAsExpression",
      expression: {
        type: "ParenthesizedExpression",
        expression: { type: "StringLiteral", value: "hi" },
      },
    };
    const result = unwrapExpression(node);
    expect(result?.type).toBe("StringLiteral");
  });
});

describe("getPropertyName", () => {
  it("extracts Identifier name", () => {
    expect(getPropertyName({ type: "Identifier", name: "foo" })).toBe("foo");
  });

  it("extracts StringLiteral value", () => {
    expect(getPropertyName({ type: "StringLiteral", value: "bar" })).toBe("bar");
  });

  it("extracts NumericLiteral value as string", () => {
    expect(getPropertyName({ type: "NumericLiteral", value: 42 })).toBe("42");
  });

  it("returns null for unsupported types", () => {
    expect(getPropertyName({ type: "ComputedProperty" })).toBeNull();
    expect(getPropertyName(null)).toBeNull();
    expect(getPropertyName(undefined)).toBeNull();
  });
});
