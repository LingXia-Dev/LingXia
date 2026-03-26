import { parse } from "@babel/parser";
import { generate } from "@babel/generator";
import {
  type BabelNode,
  AST_PARSE_OPTIONS,
  PAGE_LIFECYCLE_NAMES,
  traverseAst,
  isPageCall,
  unwrapExpression,
  getPropertyName,
} from "./ast-helpers.js";

export interface TransformPageRegistrationOptions {
  logicContent: string;
  pagePath: string;
  pluginId?: string;
}

export interface TransformAppRegistrationOptions {
  logicContent: string;
}

export function transformPageRegistration(
  options: TransformPageRegistrationOptions,
): string {
  const { logicContent, pagePath, pluginId } = options;
  const finalPath = pluginId ? `plugin/${pluginId}/${pagePath}` : pagePath;
  const ast = parse(logicContent, AST_PARSE_OPTIONS);
  let modified = false;

  traverseAst(ast.program as BabelNode, (node) => {
    if (!isPageCall(node)) return;

    const args: BabelNode[] = node.arguments ?? [];
    const pageConfig = args[0];
    if (!pageConfig) {
      throw new Error(
        `Page() must be called with a configuration expression for '${finalPath}'.`,
      );
    }
    const pageObject = unwrapExpression(pageConfig);
    const bindingMetaJson =
      pageObject?.type === "ObjectExpression"
        ? JSON.stringify(collectPageBindingMeta(pageObject))
        : JSON.stringify({ handlers: [] });

    node.callee = globalMember("__registerPage");
    node.arguments = [
      stringLiteral(finalPath),
      pageConfig,
      stringLiteral(bindingMetaJson),
    ];
    modified = true;
  });

  if (!modified) {
    throw new Error(
      `No Page() registration found while transforming '${finalPath}'.`,
    );
  }

  return generate(ast, { retainLines: true }).code;
}

export function transformAppRegistration(
  options: TransformAppRegistrationOptions,
): string {
  const { logicContent } = options;
  const ast = parse(logicContent, AST_PARSE_OPTIONS);
  let modified = false;

  traverseAst(ast.program as BabelNode, (node) => {
    if (!isAppCall(node)) return;

    const args: BabelNode[] = node.arguments ?? [];
    const appConfig = args[0];
    if (!appConfig) {
      throw new Error("App() must be called with a configuration object.");
    }
    const appObject = unwrapExpression(appConfig);
    const handlerNamesJson =
      appObject?.type === "ObjectExpression"
        ? JSON.stringify(collectAppHandlerNames(appObject))
        : JSON.stringify([]);

    node.callee = globalMember("__registerApp");
    node.arguments = [
      appConfig,
      stringLiteral(handlerNamesJson),
    ];
    modified = true;
  });

  if (!modified) {
    return logicContent;
  }

  return generate(ast, { retainLines: true }).code;
}

function collectPageBindingMeta(node: BabelNode): { handlers: string[] } {
  return {
    handlers: collectPageHandlerNames(node),
  };
}

function collectPageHandlerNames(node: BabelNode): string[] {
  const names = new Set<string>();
  const properties = node.properties as BabelNode[] | undefined;
  if (!properties) return [];

  for (const prop of properties) {
    if (!prop || prop.type === "SpreadElement") continue;
    if (!isFunctionLikeProperty(prop)) continue;
    const name = getPropertyName(prop.key as BabelNode);
    if (!name || name === "data" || name.startsWith("_")) continue;
    if (PAGE_LIFECYCLE_NAMES.has(name)) continue;
    names.add(name);
  }

  return Array.from(names);
}

function collectAppHandlerNames(node: BabelNode): string[] {
  const lifecycleNames = new Set([
    "onLaunch",
    "onShow",
    "onHide",
    "onUserCaptureScreen",
  ]);
  const names = new Set<string>();
  const properties = node.properties as BabelNode[] | undefined;
  if (!properties) return [];

  for (const prop of properties) {
    if (!prop || prop.type === "SpreadElement") continue;
    if (!isFunctionLikeProperty(prop)) continue;
    const name = getPropertyName(prop.key as BabelNode);
    if (!name || name.startsWith("_") || !lifecycleNames.has(name)) continue;
    names.add(name);
  }

  return Array.from(names);
}

function isFunctionLikeProperty(node: BabelNode): boolean {
  if (node.type === "ObjectMethod") {
    return true;
  }

  if (node.type !== "ObjectProperty") {
    return false;
  }

  const value = unwrapExpression(node.value as BabelNode | undefined);
  const propertyName = getPropertyName(node.key as BabelNode | undefined);
  return Boolean(
    value &&
      (value.type === "FunctionExpression" ||
        value.type === "ArrowFunctionExpression" ||
        (value.type === "Identifier" &&
          (node.shorthand === true || value.name === propertyName)) ||
        value.type === "MemberExpression"),
  );
}

const isAppCall = (node: BabelNode): boolean => {
  if (node.type !== "CallExpression") {
    return false;
  }
  const callee = node.callee as BabelNode | undefined;
  return Boolean(callee && callee.type === "Identifier" && callee.name === "App");
};

const stringLiteral = (value: string): BabelNode => ({
  type: "StringLiteral",
  value,
});

const globalMember = (property: string): BabelNode => ({
  type: "MemberExpression",
  object: {
    type: "Identifier",
    name: "globalThis",
  },
  property: {
    type: "Identifier",
    name: property,
  },
  computed: false,
  optional: false,
});
