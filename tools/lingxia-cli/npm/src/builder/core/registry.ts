/**
 * Framework Registry - Centralized framework configuration
 * Makes it easy to add new frameworks without modifying multiple files
 */

export interface FrameworkConfig {
  name: string;
  extensions: string[];
  mainTemplate: string;
  indexTemplate: string;
  vitePlugin?: string;
  hasComponents: boolean;
}

/**
 * Centralized registry of all supported frameworks
 */
export class FrameworkRegistry {
  private static frameworks: Map<string, FrameworkConfig> = new Map([
    [
      "react",
      {
        name: "React",
        extensions: [".tsx", ".jsx"],
        mainTemplate: "main.jsx",
        indexTemplate: "index.html",
        vitePlugin: "@vitejs/plugin-react",
        hasComponents: true,
      },
    ],
    [
      "vue",
      {
        name: "Vue",
        extensions: [".vue"],
        mainTemplate: "main.js",
        indexTemplate: "index.html",
        vitePlugin: "@vitejs/plugin-vue",
        hasComponents: true,
      },
    ],
    [
      "html",
      {
        name: "HTML",
        extensions: [".html"],
        mainTemplate: "",
        indexTemplate: "",
        hasComponents: false,
      },
    ],
  ]);

  /**
   * Get all supported frameworks
   */
  static getAllFrameworks(): string[] {
    return Array.from(this.frameworks.keys());
  }

  /**
   * Get framework configuration
   */
  static getFramework(name: string): FrameworkConfig | undefined {
    return this.frameworks.get(name);
  }

  /**
   * Detect framework from file extension
   */
  static detectFramework(filePath: string): string {
    const ext = filePath.substring(filePath.lastIndexOf("."));

    for (const [frameworkName, config] of this.frameworks) {
      if (config.extensions.includes(ext)) {
        return frameworkName;
      }
    }

    return "html"; // Default fallback
  }

  /**
   * Check if framework is supported
   */
  static isSupported(framework: string): boolean {
    return this.frameworks.has(framework);
  }

  /**
   * Register a new framework (for future extensibility)
   */
  static register(name: string, config: FrameworkConfig): void {
    this.frameworks.set(name, config);
  }

  /**
   * Get frameworks that need Vite building
   */
  static getBuildableFrameworks(): string[] {
    return Array.from(this.frameworks.entries())
      .filter(([_, config]) => config.hasComponents)
      .map(([name, _]) => name);
  }
}
