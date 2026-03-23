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

    const functionBridge = this.generateFunctionBridge(pageFunctions);
    return templates.mainEntry.replace(
      "/* {{PAGE_FUNCTIONS}} */",
      functionBridge,
    );
  }

  generateFunctionBridge(functions: PageBridgeMethod[] | string[]): string {
    if (functions.length === 0) {
      return "window.__PAGE_FUNCTIONS = [];";
    }

    const normalized = functions.map((entry) =>
      typeof entry === "string"
        ? { name: entry, mode: "notify" as PageBridgeMode }
        : entry,
    );
    const functionList = JSON.stringify(normalized.map((entry) => entry.name));

    // Generate explicit function wrappers for better debugging and runtime safety
    const wrappers = normalized
      .map(
        ({ name: funcName, mode }) => `
window['${funcName}'] = function(...args) {
  // Filter out React/DOM event objects to prevent circular reference errors
  const cleanArgs = args.filter(arg => {
    if (arg && typeof arg === 'object') {
      return !(arg.nativeEvent || arg.target || arg.currentTarget ||
               arg instanceof Event || arg.constructor.name.includes('Event'));
    }
    return true;
  });

  try {
    if (cleanArgs.length > 1) {
      throw new Error("Page action '${funcName}' accepts at most one payload argument");
    }
    const payload = cleanArgs.length === 0 ? undefined : cleanArgs[0];
    ${
      mode === "stream"
        ? `return window.LingXiaBridge.callStream('${funcName}', payload);`
        : mode === "call"
          ? `return window.LingXiaBridge.call('${funcName}', payload);`
          : `window.LingXiaBridge.notify('${funcName}', payload);`
    }
  } catch (e) {
    console.warn('[PageFunc] ${funcName} failed:', e && e.message ? e.message : e);
    throw e;
  }
};`,
      )
      .join("\n");

    return `window.__PAGE_FUNCTIONS = ${functionList};

// Generate bridge functions
${wrappers}`;
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
