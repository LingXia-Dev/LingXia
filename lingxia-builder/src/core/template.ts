import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';
import { FrameworkRegistry } from './registry.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

/**
 * Manages all template operations for the LingXia builder
 * Handles template copying, processing, and framework-specific configurations
 */
export class TemplateManager {
  private templatesDir: string;

  constructor() {
    // Navigate from dist/core to src/templates
    this.templatesDir = path.join(__dirname, '../../src/templates');
  }

  /**
   * Get templates directory path
   */
  getTemplatesDir(): string {
    return this.templatesDir;
  }

  /**
   * Copy framework-specific templates to build directory
   */
  copyFrameworkTemplates(framework: string, buildDir: string): void {
    // Validate framework
    if (!FrameworkRegistry.isSupported(framework)) {
      throw new Error(`Unsupported framework: ${framework}`);
    }
    const frameworkTemplateDir = path.join(this.templatesDir, framework);

    if (!fs.existsSync(frameworkTemplateDir)) {
      throw new Error(`Framework templates not found: ${framework}`);
    }

    // Copy all template files
    const templateFiles = fs.readdirSync(frameworkTemplateDir);
    for (const file of templateFiles) {
      const sourcePath = path.join(frameworkTemplateDir, file);
      const destPath = path.join(buildDir, file);

      if (fs.statSync(sourcePath).isFile()) {
        fs.copyFileSync(sourcePath, destPath);
      }
    }
  }

  /**
   * Get appropriate Vite config based on project setup and build mode
   */
  getViteConfig(framework: string, projectPath: string, isProd: boolean = false): string {
    if (framework === 'logic') {
      // Return logic-specific Vite config
      return `export default {
  build: {
    lib: {
      entry: 'main.js',
      name: 'LingXiaLogic',
      fileName: 'main',
      formats: ['iife']
    },
    outDir: 'dist',
    emptyOutDir: true,
    minify: ${isProd ? 'true' : 'false'},
    sourcemap: ${isProd ? 'false' : 'true'}
  }
};`;
    }

    // Validate framework
    if (!FrameworkRegistry.isSupported(framework)) {
      throw new Error(`Unsupported framework: ${framework}`);
    }

    const hasTailwind = fs.existsSync(path.join(projectPath, 'tailwind.config.js'));
    const configFileName = hasTailwind ? 'vite.config.tailwind.js' : 'vite.config.js';
    const configPath = path.join(this.templatesDir, framework, configFileName);

    let configContent: string;
    if (!fs.existsSync(configPath)) {
      // Fallback to basic config
      const basicConfigPath = path.join(this.templatesDir, framework, 'vite.config.js');
      configContent = fs.readFileSync(basicConfigPath, 'utf-8');
    } else {
      configContent = fs.readFileSync(configPath, 'utf-8');
    }

    // Apply production optimizations
    if (isProd) {
      configContent = configContent.replace(/minify:\s*false/g, 'minify: true');
      configContent = configContent.replace(/sourcemap:\s*true/g, 'sourcemap: false');
      // Add tree shaking and other optimizations
      if (!configContent.includes('minify:')) {
        configContent = configContent.replace(
          /build:\s*{/,
          `build: {
    minify: true,
    sourcemap: false,`
        );
      }
    }

    return configContent;
  }

  /**
   * Get framework dependencies from template
   */
  getFrameworkDependencies(framework: string): any {
    const depsPath = path.join(this.templatesDir, 'dependencies.json');
    const dependencies = JSON.parse(fs.readFileSync(depsPath, 'utf-8'));

    if (framework === 'logic') {
      return { devDependencies: dependencies.common.devDependencies };
    }

    // Validate framework
    if (!FrameworkRegistry.isSupported(framework)) {
      throw new Error(`Unsupported framework: ${framework}`);
    }

    const frameworkDeps = dependencies.frameworks[framework] || {};

    // Merge common dependencies with framework-specific ones
    return {
      dependencies: frameworkDeps.dependencies || {},
      devDependencies: {
        ...dependencies.common.devDependencies,
        ...frameworkDeps.devDependencies
      }
    };
  }

  /**
   * Get complete dependency configuration
   */
  getDependencyConfig(): any {
    const depsPath = path.join(this.templatesDir, 'dependencies.json');
    return JSON.parse(fs.readFileSync(depsPath, 'utf-8'));
  }

  /**
   * Generate page template with injected functions
   */
  generatePageTemplate(framework: 'react' | 'vue', pageFunctions: string[]): string {
    const templatePath = path.join(this.templatesDir, framework, 'main.jsx');
    let template = fs.readFileSync(templatePath, 'utf-8');

    // Inject page functions
    const functionBridge = this.generateFunctionBridge(pageFunctions);
    template = template.replace('{{PAGE_FUNCTIONS}}', functionBridge);

    return template;
  }

  /**
   * Create package.json for framework builds
   */
  createPackageJson(framework: string, buildDir: string, projectPath: string): void {
    const basePackageJson: any = {
      name: `lingxia-${framework}-build`,
      version: '1.0.0',
      type: 'module',
      scripts: {
        build: 'vite build'
      }
    };

    // Get framework-specific dependencies
    const frameworkDeps = this.getFrameworkDependencies(framework);

    // Start with framework dependencies
    if (frameworkDeps.dependencies) {
      basePackageJson.dependencies = { ...frameworkDeps.dependencies };
    }
    if (frameworkDeps.devDependencies) {
      basePackageJson.devDependencies = frameworkDeps.devDependencies;
    }

    // Merge project dependencies if they exist (project deps override framework deps)
    const projectPackageJson = path.join(projectPath, 'package.json');
    if (fs.existsSync(projectPackageJson)) {
      const projectDeps = JSON.parse(fs.readFileSync(projectPackageJson, 'utf-8'));
      if (projectDeps.dependencies) {
        basePackageJson.dependencies = {
          ...basePackageJson.dependencies,
          ...projectDeps.dependencies
        };
      }
    }

    fs.writeFileSync(path.join(buildDir, 'package.json'), JSON.stringify(basePackageJson, null, 2));
  }

  /**
   * Generate function bridge code for page templates
   * Note: Functions are already filtered in ViewBuilder.extractPageFunctions
   */
  generateFunctionBridge(functions: string[]): string {
    if (functions.length === 0) {
      return 'window.__PAGE_FUNCTIONS = [];';
    }

    const functionList = JSON.stringify(functions);

    return `window.__PAGE_FUNCTIONS = ${functionList};

// Generate bridge functions
window.__PAGE_FUNCTIONS.forEach(function(funcName) {
  window[funcName] = function(...args) {
    return window.LingXiaBridge.call(funcName, args.length === 1 ? args[0] : args);
  };
});`;
  }

  /**
   * Check if template exists for framework
   */
  hasFrameworkTemplate(framework: string): boolean {
    const frameworkDir = path.join(this.templatesDir, framework);
    return fs.existsSync(frameworkDir);
  }
}
