import * as path from "path";
import { FrameworkProcessor } from "./base.js";
import { ReactProcessor } from "./react.js";
import { VueProcessor } from "./vue.js";
import { HtmlProcessor } from "./html.js";

/**
 * Framework Factory - Creates appropriate framework processor
 * Adding new frameworks only requires adding them here
 */
export class FrameworkFactory {
  private static processors = new Map<
    string,
    new (projectPath: string, outputDir: string) => FrameworkProcessor
  >([
    ["react", ReactProcessor],
    ["vue", VueProcessor],
    ["html", HtmlProcessor],
  ]);

  /**
   * Create framework processor for given framework
   */
  static createProcessor(
    framework: string,
    projectPath: string,
    outputDir: string,
  ): FrameworkProcessor {
    const ProcessorClass = this.processors.get(framework.toLowerCase());

    if (!ProcessorClass) {
      throw new Error(`Unsupported framework: ${framework}`);
    }

    return new ProcessorClass(projectPath, outputDir);
  }

  /**
   * Detect framework from file path
   */
  static detectFramework(filePath: string): string {
    const ext = path.extname(filePath).toLowerCase();

    // Create temporary processors to check extensions
    for (const [frameworkName, ProcessorClass] of this.processors) {
      const tempProcessor = new ProcessorClass("", "");
      if (tempProcessor.getExtensions().includes(ext)) {
        return frameworkName;
      }
    }

    return "html"; // Default fallback
  }

  /**
   * Get all supported frameworks
   */
  static getSupportedFrameworks(): string[] {
    return Array.from(this.processors.keys());
  }

  /**
   * Register a new framework processor
   */
  static registerFramework(
    name: string,
    processorClass: new (
      projectPath: string,
      outputDir: string,
    ) => FrameworkProcessor,
  ): void {
    this.processors.set(name.toLowerCase(), processorClass);
  }

  /**
   * Check if framework is supported
   */
  static isSupported(framework: string): boolean {
    return this.processors.has(framework.toLowerCase());
  }
}
