import { parse, type ParserOptions } from "@babel/parser";

const LIFECYCLE_FUNCTIONS = new Set([
  "onLoad",
  "onShow",
  "onHide",
  "onUnload",
  "onReady",
  "onPullDownRefresh",
  "onReachBottom",
  "onShareAppMessage",
  "onPageScroll",
  "onTabItemTap",
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

export interface TypeInfo {
  type: string; // "string" | "number" | "boolean" | "object" | "array" | "null" | "unknown"
  optional?: boolean;
  elementType?: TypeInfo; // for arrays
  properties?: Record<string, TypeInfo>; // for nested objects
}

export interface MethodParam {
  name: string;
  type: string;
  optional?: boolean;
}

export interface MethodInfo {
  params: MethodParam[];
  returnType?: string;
  async?: boolean;
}

export interface PageTypeInfo {
  data: Record<string, TypeInfo>;
  methods: Record<string, MethodInfo>;
}

type BabelNode = {
  type?: string;
  [key: string]: unknown;
};

export function extractPageTypes(source: string): PageTypeInfo {
  const ast = parse(source, AST_PARSE_OPTIONS);
  const result: PageTypeInfo = { data: {}, methods: {} };

  traverseAst(ast.program as unknown as BabelNode, (node) => {
    if (!isPageCall(node)) return;

    const firstArg = unwrapExpression(node.arguments?.[0] as BabelNode | undefined);
    if (firstArg?.type === "ObjectExpression") {
      extractFromPageObject(firstArg, result);
    }
  });

  return result;
}

function extractFromPageObject(node: BabelNode, result: PageTypeInfo): void {
  const properties = node.properties as BabelNode[] | undefined;
  if (!properties) return;

  for (const prop of properties) {
    if (!prop || prop.type === "SpreadElement") continue;

    const name = getPropertyName(prop.key as BabelNode);
    if (!name || name.startsWith("_")) continue;

    // Extract data object
    if (name === "data" && prop.type === "ObjectProperty") {
      const valueNode = unwrapExpression(prop.value as BabelNode);
      if (valueNode?.type === "ObjectExpression") {
        result.data = extractDataTypes(valueNode);
      }
      continue;
    }

    // Skip lifecycle functions
    if (LIFECYCLE_FUNCTIONS.has(name)) continue;

    // Extract method
    if (prop.type === "ObjectMethod") {
      result.methods[name] = extractMethodInfo(prop);
    } else if (prop.type === "ObjectProperty") {
      const valueNode = unwrapExpression(prop.value as BabelNode);
      if (
        valueNode &&
        (valueNode.type === "FunctionExpression" ||
          valueNode.type === "ArrowFunctionExpression")
      ) {
        result.methods[name] = extractMethodInfo(valueNode);
      }
    }
  }
}

function extractDataTypes(node: BabelNode): Record<string, TypeInfo> {
  const result: Record<string, TypeInfo> = {};
  const properties = node.properties as BabelNode[] | undefined;
  if (!properties) return result;

  for (const prop of properties) {
    if (!prop || prop.type === "SpreadElement") continue;

    const name = getPropertyName(prop.key as BabelNode);
    if (!name) continue;

    if (prop.type === "ObjectProperty") {
      const valueNode = unwrapExpression(prop.value as BabelNode);
      result[name] = inferTypeFromValue(valueNode);
    }
  }

  return result;
}

function inferTypeFromValue(node: BabelNode | null | undefined): TypeInfo {
  if (!node) return { type: "unknown" };

  switch (node.type) {
    case "StringLiteral":
      return { type: "string" };
    case "NumericLiteral":
      return { type: "number" };
    case "BooleanLiteral":
      return { type: "boolean" };
    case "NullLiteral":
      return { type: "null", optional: true };
    case "ArrayExpression": {
      const elements = node.elements as BabelNode[] | undefined;
      if (elements && elements.length > 0) {
        const firstElement = unwrapExpression(elements[0]);
        return { type: "array", elementType: inferTypeFromValue(firstElement) };
      }
      return { type: "array", elementType: { type: "unknown" } };
    }
    case "ObjectExpression": {
      const properties = extractDataTypes(node);
      return { type: "object", properties };
    }
    case "TemplateLiteral":
      return { type: "string" };
    case "UnaryExpression":
      // Handle negative numbers: -1
      if (node.operator === "-" || node.operator === "+") {
        return { type: "number" };
      }
      return { type: "unknown" };
    default:
      return { type: "unknown" };
  }
}

function extractMethodInfo(node: BabelNode): MethodInfo {
  const params: MethodParam[] = [];
  const nodeParams = node.params as BabelNode[] | undefined;

  if (nodeParams) {
    for (const param of nodeParams) {
      const paramInfo = extractParamInfo(param);
      if (paramInfo) params.push(paramInfo);
    }
  }

  const isAsync = Boolean(node.async);
  const returnType = extractReturnType(node);

  return { params, async: isAsync, returnType };
}

function extractParamInfo(param: BabelNode): MethodParam | null {
  if (!param) return null;

  // Handle simple identifier: function foo(arg) {}
  if (param.type === "Identifier") {
    const typeAnnotation = param.typeAnnotation as BabelNode | undefined;
    return {
      name: param.name as string,
      type: extractTypeAnnotation(typeAnnotation),
      optional: Boolean(param.optional),
    };
  }

  // Handle assignment pattern: function foo(arg = defaultValue) {}
  if (param.type === "AssignmentPattern") {
    const left = param.left as BabelNode;
    if (left?.type === "Identifier") {
      const typeAnnotation = left.typeAnnotation as BabelNode | undefined;
      const right = param.right as BabelNode;
      return {
        name: left.name as string,
        type: typeAnnotation
          ? extractTypeAnnotation(typeAnnotation)
          : inferTypeFromValue(right).type,
        optional: true,
      };
    }
  }

  // Handle rest parameter: function foo(...args) {}
  if (param.type === "RestElement") {
    const argument = param.argument as BabelNode;
    if (argument?.type === "Identifier") {
      const typeAnnotation = param.typeAnnotation as BabelNode | undefined;
      return {
        name: `...${argument.name as string}`,
        type: extractTypeAnnotation(typeAnnotation) || "unknown[]",
        optional: true,
      };
    }
  }

  // Handle object destructuring: function foo({ name, age }) {}
  if (param.type === "ObjectPattern") {
    const typeAnnotation = param.typeAnnotation as BabelNode | undefined;
    return {
      name: "options",
      type: extractTypeAnnotation(typeAnnotation) || "object",
      optional: Boolean(param.optional),
    };
  }

  return null;
}

function extractTypeAnnotation(typeAnnotation: BabelNode | undefined): string {
  if (!typeAnnotation) return "unknown";

  // Handle TSTypeAnnotation wrapper
  if (typeAnnotation.type === "TSTypeAnnotation") {
    return extractTSType(typeAnnotation.typeAnnotation as BabelNode);
  }

  return extractTSType(typeAnnotation);
}

function extractTSType(node: BabelNode | undefined): string {
  if (!node) return "unknown";

  switch (node.type) {
    case "TSStringKeyword":
      return "string";
    case "TSNumberKeyword":
      return "number";
    case "TSBooleanKeyword":
      return "boolean";
    case "TSNullKeyword":
      return "null";
    case "TSUndefinedKeyword":
      return "undefined";
    case "TSVoidKeyword":
      return "void";
    case "TSAnyKeyword":
      return "unknown";
    case "TSUnknownKeyword":
      return "unknown";
    case "TSNeverKeyword":
      return "never";
    case "TSObjectKeyword":
      return "object";
    case "TSArrayType": {
      const elementType = extractTSType(node.elementType as BabelNode);
      return `${elementType}[]`;
    }
    case "TSTypeReference": {
      const typeName = node.typeName as BabelNode;
      if (typeName?.type === "Identifier") {
        const name = typeName.name as string;
        const typeParams = node.typeParameters as BabelNode | undefined;
        if (typeParams?.type === "TSTypeParameterInstantiation") {
          const params = (typeParams.params as BabelNode[])
            .map((p) => extractTSType(p))
            .join(", ");
          return `${name}<${params}>`;
        }
        return name;
      }
      return "unknown";
    }
    case "TSUnionType": {
      const types = (node.types as BabelNode[]).map((t) => extractTSType(t));
      return types.join(" | ");
    }
    case "TSIntersectionType": {
      const types = (node.types as BabelNode[]).map((t) => extractTSType(t));
      return types.join(" & ");
    }
    case "TSLiteralType": {
      const literal = node.literal as BabelNode;
      if (literal?.type === "StringLiteral") return `"${literal.value}"`;
      if (literal?.type === "NumericLiteral") return String(literal.value);
      if (literal?.type === "BooleanLiteral") return String(literal.value);
      return "unknown";
    }
    case "TSTypeLiteral": {
      // Inline object type: { name: string; age: number }
      const members = node.members as BabelNode[] | undefined;
      if (!members || members.length === 0) return "{}";
      const props = members
        .map((m) => {
          if (m.type === "TSPropertySignature") {
            const key = getPropertyName(m.key as BabelNode);
            const typeAnn = m.typeAnnotation as BabelNode | undefined;
            const propType = extractTypeAnnotation(typeAnn);
            const optional = m.optional ? "?" : "";
            return `${key}${optional}: ${propType}`;
          }
          return null;
        })
        .filter(Boolean);
      return `{ ${props.join("; ")} }`;
    }
    case "TSFunctionType": {
      const params = (node.parameters as BabelNode[] | undefined) || [];
      const paramStrs = params.map((p) => {
        const name = (p as BabelNode & { name?: string }).name || "arg";
        const typeAnn = (p as BabelNode).typeAnnotation as BabelNode | undefined;
        return `${name}: ${extractTypeAnnotation(typeAnn)}`;
      });
      const returnType = extractTypeAnnotation(node.typeAnnotation as BabelNode);
      return `(${paramStrs.join(", ")}) => ${returnType}`;
    }
    case "TSParenthesizedType":
      return `(${extractTSType(node.typeAnnotation as BabelNode)})`;
    case "TSTupleType": {
      const elementTypes = (node.elementTypes as BabelNode[]).map((t) =>
        extractTSType(t)
      );
      return `[${elementTypes.join(", ")}]`;
    }
    default:
      return "unknown";
  }
}

function extractReturnType(node: BabelNode): string | undefined {
  const returnType = node.returnType as BabelNode | undefined;
  if (!returnType) return undefined;

  if (returnType.type === "TSTypeAnnotation") {
    const type = extractTSType(returnType.typeAnnotation as BabelNode);
    // Don't return void as it's the default
    if (type === "void") return undefined;
    return type;
  }

  return undefined;
}

// Helper functions (same as page-functions.ts)

function traverseAst(
  node: BabelNode | null | undefined,
  visitor: (node: BabelNode) => void
): void {
  if (!node || typeof node.type !== "string") return;

  visitor(node);

  for (const value of Object.values(node)) {
    if (!value) continue;

    if (Array.isArray(value)) {
      for (const child of value) {
        if (child && typeof (child as BabelNode).type === "string") {
          traverseAst(child as BabelNode, visitor);
        }
      }
      continue;
    }

    if (value && typeof (value as BabelNode).type === "string") {
      traverseAst(value as BabelNode, visitor);
    }
  }
}

function isPageCall(node: BabelNode): node is BabelNode {
  if (node.type !== "CallExpression") return false;
  const callee = node.callee as BabelNode | undefined;
  return Boolean(callee && callee.type === "Identifier" && callee.name === "Page");
}

function unwrapExpression(node?: BabelNode | null): BabelNode | null {
  let current: BabelNode | null = node ?? null;

  while (current) {
    if (current.type === "SpreadElement") return null;

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
}

function getPropertyName(key?: BabelNode | null): string | null {
  if (!key) return null;

  if (key.type === "Identifier") return key.name as string;
  if (key.type === "StringLiteral") return key.value as string;
  if (key.type === "NumericLiteral") return String(key.value);

  return null;
}
