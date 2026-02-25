import * as fs from "fs";
import * as path from "path";
import { FrameworkRegistry } from "./registry.js";
import {
  hasFrameworkTemplates,
  getFrameworkTemplates,
} from "./framework-templates.js";

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
    pageFunctions: string[],
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

  generateFunctionBridge(functions: string[]): string {
    if (functions.length === 0) {
      return "window.__PAGE_FUNCTIONS = [];";
    }

    const functionList = JSON.stringify(functions);

    // Generate explicit function wrappers for better debugging and runtime safety
    const wrappers = functions
      .map(
        (funcName) => `
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
    // Page functions are fire-and-forget. Business results should be expressed via state updates
    // (LXS / setData), not via req/res return values.
    window.LingXiaBridge.notify('${funcName}', cleanArgs.length === 1 ? cleanArgs[0] : cleanArgs);
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

  hasFrameworkTemplate(framework: string): boolean {
    return hasFrameworkTemplates(framework);
  }
}
