import * as fs from "fs";
import * as path from "path";
import { FrameworkRegistry } from "./registry.js";
import {
  hasFrameworkTemplates,
  getFrameworkTemplates,
} from "./framework-templates.js";
import type { MethodInfo } from "./builders/page-types.js";

export type PageBridgeMode = "notify" | "call" | "stream";

export interface PageBridgeMethod {
  name: string;
  mode: PageBridgeMode;
}

export function assertBridgeMethodCompatible(
  name: string,
  info: MethodInfo,
): void {
  if (info.params.length > 1) {
    throw new Error(
      `Page action '${name}' must accept zero or one payload parameter; ` +
        `found ${info.params.length}. Wrap multiple values in a single object.`,
    );
  }
}


function normalizeBridgeMethods(
  functions: PageBridgeMethod[] | string[],
): PageBridgeMethod[] {
  return functions.map((entry) =>
    typeof entry === "string"
      ? { name: entry, mode: "notify" as PageBridgeMode }
      : entry,
  );
}

/**
 * Payload filter function source lines (no leading indent).
 * Filters out React/DOM event objects and enforces single-payload constraint.
 */
const PAYLOAD_FILTER_LINES = [
  `function _fp(name, args) {`,
  `  var clean = [];`,
  `  for (var i = 0; i < args.length; i++) {`,
  `    var a = args[i];`,
  `    if (a instanceof Event) continue;`,
  `    if (a && typeof a === 'object' && typeof a.stopPropagation === 'function') continue;`,
  `    clean.push(a);`,
  `  }`,
  `  if (clean.length > 1) throw new Error("Page action '" + name + "' accepts at most one payload argument");`,
  `  return clean.length === 0 ? undefined : clean[0];`,
  `}`,
];

function indentPayloadFilter(indent: string): string {
  return PAYLOAD_FILTER_LINES.map((l) => indent + l).join("\n");
}

function bridgeCallExpression(funcName: string, mode: PageBridgeMode): string {
  if (mode === "stream") {
    return `return window.LingXiaBridge.callStream('${funcName}', payload);`;
  }
  if (mode === "call") {
    return `return window.LingXiaBridge.call('${funcName}', payload);`;
  }
  return `window.LingXiaBridge.notify('${funcName}', payload);`;
}


export class TemplateManager {
  copyFrameworkTemplates(framework: string, buildDir: string): void {
    if (!FrameworkRegistry.isSupported(framework)) {
      throw new Error(`Unsupported framework: ${framework}`);
    }

    const templates = getFrameworkTemplates(framework);
    if (!templates) {
      throw new Error(`Framework templates not found: ${framework}`);
    }

    fs.writeFileSync(path.join(buildDir, "index.html"), templates.indexHtml);
    fs.writeFileSync(
      path.join(buildDir, templates.mainEntryFilename),
      templates.mainEntry,
    );
  }

  generatePageTemplate(
    framework: "react" | "vue",
    pageFunctions: PageBridgeMethod[],
  ): string {
    const templates = getFrameworkTemplates(framework);
    if (!templates) {
      throw new Error(`Framework templates not found: ${framework}`);
    }

    // For standalone template generation, inline the bridge as window.__pageBridge
    const bridgeCode = this.generateFunctionBridge(pageFunctions);
    return templates.mainEntry.replace(
      "/* {{PAGE_BRIDGE_IMPORT}} */",
      bridgeCode,
    );
  }

  /**
   * Generate an inline `<script>` body that defines `window.__pageBridge`.
   * Used for HTML pages (no bundler) and as the post-build inline fallback
   * for React/Vue pages.
   */
  generateFunctionBridge(functions: PageBridgeMethod[] | string[]): string {
    if (functions.length === 0) {
      return "window.__pageBridge = { __names: [] };";
    }

    const normalized = normalizeBridgeMethods(functions);
    const nameList = JSON.stringify(normalized.map((entry) => entry.name));

    const wrappers = normalized
      .map(
        ({ name: funcName, mode }) => `
  var ${funcName} = function(...args) {
    var payload = _fp('${funcName}', args);
    ${bridgeCallExpression(funcName, mode)}
  };
  ${funcName}.__logicFunc = true;
  ${funcName}.__funcName = '${funcName}';
  ${funcName}.__bridgeMode = '${mode}';`,
      )
      .join("\n");

    const exports = normalized.map((e) => e.name).join(", ");

    return `window.__pageBridge = (function() {
${indentPayloadFilter("  ")}
${wrappers}

  return { __names: ${nameList}, ${exports} };
})();`;
  }

  /**
   * Generate a metadata-only inline script (just the names array).
   * Used for the inline `<script>` in React/Vue builds where the full
   * bridge is already bundled in the module via __page_bridge__ import.
   */
  generateBridgeMetadata(functions: PageBridgeMethod[]): string {
    if (functions.length === 0) {
      return "window.__pageBridge = { __names: [] };";
    }
    const nameList = JSON.stringify(functions.map((f) => f.name));
    return `window.__pageBridge = { __names: ${nameList} };`;
  }

  /**
   * Generate an ES module file that exports each bridge function as a
   * named export.  Written to `__page_bridge__.js` in the Vite build
   * directory so the entry module can `import * as` and re-export onto
   * `window.__pageBridge`.
   */
  generatePageBridgeModule(functions: PageBridgeMethod[]): string {
    if (functions.length === 0) {
      return "export var __names = [];\n";
    }

    const lines = [
      "// Auto-generated by lingxia-cli. Do not edit.",
      indentPayloadFilter(""),
      "",
    ];

    for (const { name, mode } of functions) {
      lines.push(
        `export function ${name}(...args) {`,
        `  var payload = _fp('${name}', args);`,
        `  ${bridgeCallExpression(name, mode)}`,
        `}`,
        `${name}.__logicFunc = true;`,
        `${name}.__funcName = '${name}';`,
        `${name}.__bridgeMode = '${mode}';`,
        "",
      );
    }

    const nameList = JSON.stringify(functions.map((f) => f.name));
    lines.push(`export var __names = ${nameList};`);

    return lines.join("\n");
  }

  inferBridgeMethods(methods: Record<string, MethodInfo>): PageBridgeMethod[] {
    return Object.entries(methods).map(([name, info]) => {
      assertBridgeMethodCompatible(name, info);
      return {
        name,
        mode: inferBridgeMode(info),
      };
    });
  }

  hasFrameworkTemplate(framework: string): boolean {
    return hasFrameworkTemplates(framework);
  }
}

function inferBridgeMode(info: MethodInfo): PageBridgeMode {
  if (info.generator) {
    return "stream";
  }

  const returnType = info.returnType?.replace(/\s+/g, "");
  if (!returnType) {
    return "notify";
  }

  if (
    returnType.startsWith("AsyncIterable<") ||
    returnType.startsWith("AsyncIterator<") ||
    returnType.startsWith("AsyncGenerator<")
  ) {
    return "stream";
  }

  if (
    returnType === "void" ||
    returnType === "undefined" ||
    returnType === "Promise<void>"
  ) {
    return "notify";
  }

  return "call";
}
