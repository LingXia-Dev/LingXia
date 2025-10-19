import fs from 'fs';
import path from 'path';
import type { BuildOptions, Page } from '../types/index.js';
import { ViewBuilder } from '../core/builders/view.js';
import { LogicBuilder } from '../core/builders/logic.js';
import { FileUtils } from '../core/utils/file.js';
import { detectPageType } from '../core/utils/page.js';
import { ConfigManager } from '../core/config.js';

const fileUtils = new FileUtils();

export async function buildCommand(options: BuildOptions): Promise<void> {
  const projectPath = process.cwd();
  const outputDir = path.join(projectPath, 'dist');

  // Map CLI options to BuildOptions
  const buildOptions: BuildOptions = {
    dev: options.dev,
    prod: options.prod
  };

  console.log('🚀 Starting LingXia build...');
  console.log(` Project: ${projectPath}`);
  console.log(` Output: ${outputDir}`);
  console.log(` View bundler: Vite`);

  try {
    // Clean and prepare output directory
    fileUtils.cleanDirectory(outputDir);

    // Discover pages
    const pages = discoverPages(projectPath);
    console.log(` Found ${pages.length} pages: ${pages.map(p => p.name).join(', ')}`);

    if (pages.length === 0) {
      console.warn('⚠️ No pages found in the project');
      return;
    }

    const startTime = Date.now();

    const only = process.env.LINGXIA_ONLY?.toLowerCase();
    if (only === 'logic') {
      console.log('▶ Building logic layer only...');
      const logicBuilder = new LogicBuilder(projectPath, outputDir);
      await logicBuilder.buildLogic(buildOptions);
    } else if (only === 'view') {
      console.log('▶ Building view layer only...');
      const viewBuilder = new ViewBuilder(projectPath, outputDir);
      await viewBuilder.buildPages(pages, buildOptions);
    } else {
      
      console.log('▶ Building logic layer...');
      const logicBuilder = new LogicBuilder(projectPath, outputDir);
      await logicBuilder.buildLogic(buildOptions);

      console.log('▶ Building view layer...');
      const viewBuilder = new ViewBuilder(projectPath, outputDir);
      await viewBuilder.buildPages(pages, buildOptions);
    }

    const endTime = Date.now();
    const buildTime = ((endTime - startTime) / 1000).toFixed(2);

    console.log('✅ Build completed successfully!');
    console.log(` ⏱ Completed in ${buildTime}s`);
    console.log(` 📁 Output directory: ${outputDir}`);

  } catch (error) {
    console.error('❌ Build failed:', error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}

function discoverPages(projectPath: string): Page[] {
  const configManager = new ConfigManager(projectPath);
  const pagesPaths = configManager.getPages();

  const pages: Page[] = [];

  for (const pagePath of pagesPaths) {
    // Check if page file exists
    const fullPath = path.join(projectPath, pagePath);
    if (!fs.existsSync(fullPath)) {
      console.warn(`⚠️ Page file not found: ${pagePath}`);
      continue;
    }

    // Extract page info
    const pageType = detectPageType(pagePath);
    const pageDir = path.dirname(pagePath);
    const baseName = path.basename(pagePath, path.extname(pagePath));

    // Create page name from directory structure
    let pageName = pageDir;
    if (pageDir.startsWith('pages/')) {
      pageName = pageDir.substring(6); // Remove 'pages/' prefix
    }
    if (!pageName) {
      pageName = baseName;
    }

    pages.push({
      path: pagePath, // Full path from lxapp.json
      name: pageName,
      type: pageType
    });
  }

  return pages;
}
