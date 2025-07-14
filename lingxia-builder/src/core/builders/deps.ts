import fs from 'fs';
import path from 'path';
import type { DependencyConfig } from '../../types/index.js';
import { FileUtils } from '../utils/file.js';
import { TemplateManager } from '../template.js';

export class DependencyResolver {
  private projectPath: string;
  private fileUtils: FileUtils;
  private templateManager: TemplateManager;

  constructor(projectPath: string) {
    this.projectPath = projectPath;
    this.fileUtils = new FileUtils();
    this.templateManager = new TemplateManager();
  }

  /**
   * Get dependency configuration for framework
   */
  getDependencyConfig(): DependencyConfig {
    return this.templateManager.getDependencyConfig();
  }

  /**
   * Create package.json for framework with project dependencies
   */
  createPackageJson(framework: string, buildDir: string): void {
    const frameworkDeps = this.templateManager.getFrameworkDependencies(framework);

    const projectPackageJson = path.join(this.projectPath, 'package.json');
    let packageJson: any = {
      name: `lingxia-${framework}-page`,
      version: '1.0.0',
      type: 'module'
    };

    if (fs.existsSync(projectPackageJson)) {
      const projectDeps = this.fileUtils.readJsonFile(projectPackageJson);
      if (projectDeps) {
        packageJson = {
          ...packageJson,
          dependencies: { ...(frameworkDeps.dependencies || {}), ...(projectDeps.dependencies || {}) },
          devDependencies: { ...(frameworkDeps.devDependencies || {}), ...(projectDeps.devDependencies || {}) }
        };
        console.log(' Inherited project dependencies');
      }
    } else {
      packageJson = {
        ...packageJson,
        dependencies: frameworkDeps.dependencies || {},
        devDependencies: frameworkDeps.devDependencies || {}
      };
      console.log(` Using default ${framework} dependencies`);
    }

    const targetPackageJson = path.join(buildDir, 'package.json');
    this.fileUtils.writeJsonFile(targetPackageJson, packageJson);
  }

  /**
   * Resolve project dependencies in source code
   */
  async resolveProjectDependencies(content: string, buildDir: string): Promise<string> {
    const importRegex = /import\s+[^'"]*['"]\.\.\/\.\.\/([^'"]+)['"]/g;
    let match: RegExpExecArray | null;
    let resolvedContent = content;

    while ((match = importRegex.exec(content)) !== null) {
      const relativePath = match[1];
      const sourcePath = path.join(this.projectPath, relativePath);

      if (fs.existsSync(sourcePath)) {
        const fileName = path.basename(relativePath);
        const destPath = path.join(buildDir, fileName);

        fs.copyFileSync(sourcePath, destPath);

        const oldImport = match[0];
        const newImport = oldImport.replace(`../../${relativePath}`, `./${fileName}`);
        resolvedContent = resolvedContent.replace(oldImport, newImport);

        console.log(` Resolved dependency: ${relativePath} → ${fileName}`);
      }
    }

    return resolvedContent;
  }

  /**
   * Analyze HTML dependencies and copy them
   */
  async analyzeHtmlDependencies(htmlContent: string, pageDir: string, outputDir: string): Promise<void> {
    const sourcePageDir = path.join(this.projectPath, 'pages', pageDir);

    // Analyze <link> tags (CSS files)
    const linkMatches = htmlContent.match(/<link[^>]*href=["']([^"']+)["'][^>]*>/g) || [];
    for (const linkTag of linkMatches) {
      const hrefMatch = linkTag.match(/href=["']([^"']+)["']/);
      if (hrefMatch && hrefMatch[1]) {
        const href = hrefMatch[1];
        if (!href.startsWith('http') && !href.startsWith('//')) {
          await this.copyLocalResource(href, sourcePageDir, outputDir);
        }
      }
    }
  }

  private async copyLocalResource(resourcePath: string, sourceDir: string, outputDir: string): Promise<void> {
    const sourcePath = path.resolve(sourceDir, resourcePath);
    if (fs.existsSync(sourcePath)) {
      const fileName = path.basename(resourcePath);
      const destPath = path.join(outputDir, fileName);
      fs.copyFileSync(sourcePath, destPath);

      const ext = this.fileUtils.getExtension(fileName);
      console.log(`Copied ${ext.toUpperCase()} dependency: ${fileName}`);
    }
  }
}
