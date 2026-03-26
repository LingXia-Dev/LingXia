import type { ParserOptions } from "@babel/parser";

export type BabelNode = {
  type?: string;
  [key: string]: any;
};

export const AST_PARSE_OPTIONS: ParserOptions = {
  sourceType: "module",
  plugins: [
    "typescript",
    "jsx",
    "classProperties",
    "decorators-legacy",
    "dynamicImport",
    "objectRestSpread",
    "optionalChaining",
    "nullishCoalescingOperator",
    "topLevelAwait",
  ],
};

/**
 * Page lifecycle hook names that are dispatched by the runtime.
 * These are excluded from handler metadata and bridge generation.
 */
export const PAGE_LIFECYCLE_NAMES = new Set([
  "onLoad",
  "onShow",
  "onReady",
  "onHide",
  "onUnload",
  "onPullDownRefresh",
]);

export const traverseAst = (
  node: BabelNode | null | undefined,
  visitor: (node: BabelNode) => void,
): void => {
  if (!node || typeof node.type !== "string") {
    return;
  }

  visitor(node);

  for (const value of Object.values(node)) {
    if (!value) continue;

    if (Array.isArray(value)) {
      for (const child of value) {
        if (child && typeof child.type === "string") {
          traverseAst(child, visitor);
        }
      }
      continue;
    }

    if (value && typeof value.type === "string") {
      traverseAst(value, visitor);
    }
  }
};

export const isPageCall = (node: BabelNode): boolean => {
  if (node.type !== "CallExpression") {
    return false;
  }
  const callee = node.callee as BabelNode | undefined;
  return Boolean(
    callee && callee.type === "Identifier" && callee.name === "Page",
  );
};

export const unwrapExpression = (node?: BabelNode | null): BabelNode | null => {
  let current: BabelNode | null = node ?? null;

  while (current) {
    if (current.type === "SpreadElement") {
      return null;
    }

    if (
      current.type === "TSAsExpression" ||
      current.type === "TSTypeAssertion" ||
      current.type === "TSNonNullExpression" ||
      current.type === "TypeCastExpression"
    ) {
      current = current.expression as BabelNode;
      continue;
    }

    if (current.type === "ParenthesizedExpression") {
      current = current.expression as BabelNode;
      continue;
    }

    break;
  }

  return current;
};

export const getPropertyName = (key?: BabelNode | null): string | null => {
  if (!key) return null;
  if (key.type === "Identifier") return key.name as string;
  if (key.type === "StringLiteral") return key.value as string;
  if (key.type === "NumericLiteral") return String(key.value);
  return null;
};
