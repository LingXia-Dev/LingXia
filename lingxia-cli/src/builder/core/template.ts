import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';
import { FrameworkRegistry } from './registry.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const TEMPLATES_DIR = path.join(__dirname, '../../..', 'templates', 'builder');

export class TemplateManager {
  private templatesDir: string;

  constructor() {
    if (!fs.existsSync(TEMPLATES_DIR)) {
      throw new Error(`Builder templates directory missing: ${TEMPLATES_DIR}`);
    }
    this.templatesDir = TEMPLATES_DIR;
  }

  getTemplatesDir(): string {
    return this.templatesDir;
  }

  copyFrameworkTemplates(framework: string, buildDir: string): void {
    if (!FrameworkRegistry.isSupported(framework)) {
      throw new Error(`Unsupported framework: ${framework}`);
    }

    const frameworkTemplateDir = path.join(this.templatesDir, framework);
    if (!fs.existsSync(frameworkTemplateDir)) {
      throw new Error(`Framework templates not found: ${framework}`);
    }

    for (const file of fs.readdirSync(frameworkTemplateDir)) {
      const sourcePath = path.join(frameworkTemplateDir, file);
      const destPath = path.join(buildDir, file);
      if (fs.statSync(sourcePath).isFile()) {
        fs.copyFileSync(sourcePath, destPath);
      }
    }
  }

  generatePageTemplate(framework: 'react' | 'vue', pageFunctions: string[]): string {
    const templatePath = path.join(this.templatesDir, framework, 'main.jsx');
    let template = fs.readFileSync(templatePath, 'utf-8');
    const functionBridge = this.generateFunctionBridge(pageFunctions);
    return template.replace('{{PAGE_FUNCTIONS}}', functionBridge);
  }

  generateFunctionBridge(functions: string[]): string {
    if (functions.length === 0) {
      return 'window.__PAGE_FUNCTIONS = [];';
    }

    const functionList = JSON.stringify(functions);
    
    // Generate explicit function wrappers for better debugging and runtime safety
    const wrappers = functions.map(funcName => `
window['${funcName}'] = function(...args) {
  // Filter out React/DOM event objects to prevent circular reference errors
  const cleanArgs = args.filter(arg => {
    if (arg && typeof arg === 'object') {
      return !(arg.nativeEvent || arg.target || arg.currentTarget ||
               arg instanceof Event || arg.constructor.name.includes('Event'));
    }
    return true;
  });

  return window.LingXiaBridge.call('${funcName}', cleanArgs.length === 1 ? cleanArgs[0] : cleanArgs)
    .catch(function(e) {
      console.warn('[PageFunc] ${funcName} failed:', e.message || e);
      throw e;
    });
};`).join('\n');

    return `window.__PAGE_FUNCTIONS = ${functionList};

// Generate bridge functions
${wrappers}`;
  }

  hasFrameworkTemplate(framework: string): boolean {
    const frameworkDir = path.join(this.templatesDir, framework);
    return fs.existsSync(frameworkDir);
  }
}
