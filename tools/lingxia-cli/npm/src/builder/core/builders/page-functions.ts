import { parse, type ParserOptions } from "@babel/parser";

const LIFECYCLE_FUNCTIONS = new Set([
  "onLoad",
  "onShow",
  "onHide",
  "onUnload",
  "onReady",
]);

const AST_PARSE_OPTIONS: ParserOptions = {
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

export const extractPageFunctionsFromSource = (
  logicContent: string,
): string[] => {
  const ast = parse(logicContent, AST_PARSE_OPTIONS);
  const functions = new Set<string>();

  traverseAst(ast.program as BabelNode, (node) => {
    if (!isPageCall(node)) {
      return;
    }

    const firstArg = unwrapExpression(
      node.arguments?.[0] as BabelNode | undefined,
    );
    if (firstArg?.type === "ObjectExpression") {
      collectFunctionsFromObject(firstArg, functions);
    }
  });

  return Array.from(functions).filter((func) => !LIFECYCLE_FUNCTIONS.has(func));
};

type BabelNode = {
  type?: string;
  [key: string]: any;
};

const traverseAst = (
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

const isPageCall = (node: BabelNode): node is BabelNode => {
  if (node.type !== "CallExpression") {
    return false;
  }
  const callee = node.callee as BabelNode | undefined;
  return Boolean(
    callee && callee.type === "Identifier" && callee.name === "Page",
  );
};

const unwrapExpression = (node?: BabelNode | null): BabelNode | null => {
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

const collectFunctionsFromObject = (
  node: BabelNode,
  functions: Set<string>,
): void => {
  for (const prop of node.properties ?? []) {
    if (!prop) {
      continue;
    }

    if (prop.type === "SpreadElement") {
      continue;
    }

    const name = getPropertyName(prop.key as BabelNode);
    if (!name || name.startsWith("_")) {
      continue;
    }

    if (prop.type === "ObjectMethod") {
      functions.add(name);
      continue;
    }

    if (prop.type === "ObjectProperty") {
      const valueNode = unwrapExpression(prop.value as BabelNode);
      if (
        valueNode &&
        (valueNode.type === "FunctionExpression" ||
          valueNode.type === "ArrowFunctionExpression")
      ) {
        functions.add(name);
      }
    }
  }
};

const getPropertyName = (key?: BabelNode | null): string | null => {
  if (!key) {
    return null;
  }

  if (key.type === "Identifier") {
    return key.name as string;
  }

  if (key.type === "StringLiteral") {
    return key.value as string;
  }

  if (key.type === "NumericLiteral") {
    return String(key.value);
  }

  return null;
};
