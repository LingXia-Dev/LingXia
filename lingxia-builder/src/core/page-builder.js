import fs from "fs";
import path from "path";
import { execSync } from "child_process";
import { fileURLToPath } from "url";

import { detectPageType, getPageFiles, PAGE_TYPES } from "./page-detector.js";
import {
  extractPageFunctions,
  filterPageFunctions,
} from "./function-extractor.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Generate page function creation script
function generatePageFunctionScript(functionNames) {
  return `
window.__PAGE_FUNCTIONS = ${JSON.stringify(functionNames)};

// Create page function wrapper
function createPageFunction(funcName) {
  return function (...args) {
    let payload = null;
    if (args.length === 1 && typeof args[0] === "object" && args[0] !== null) {
      payload = args[0];
    } else if (args.length > 1) {
      console.warn(funcName + " called with multiple arguments, only the first object argument will be used");
      if (typeof args[0] === "object" && args[0] !== null) {
        payload = args[0];
      }
    }
    return window.LingXiaBridge.call(funcName, payload);
  };
}

// Register all page functions
window.__PAGE_FUNCTIONS.forEach(function(funcName) {
  window[funcName] = createPageFunction(funcName);
});`;
}

export class PageBuilder {
  constructor(options = {}) {
    this.options = {
      buildDir: ".lingxia-build",
      cleanup: true,
      ...options,
    };
  }

  // Build a single page
  async buildPage(pagePath, rootDir, outputDir) {
    try {
      console.log(`🔨 Building page: ${pagePath}`);

      const pageType = detectPageType(pagePath);
      const pageFiles = getPageFiles(pagePath, rootDir);

      if (!pageFiles.main.exists) {
        throw new Error(`Page file not found: ${pageFiles.main.path}`);
      }

      if (pageType === PAGE_TYPES.HTML) {
        this.processHtmlPage(pagePath, pageFiles, outputDir);
      } else {
        await this.processSpaPage(
          pagePath,
          pageType,
          pageFiles,
          rootDir,
          outputDir,
        );
      }

      console.log(`Page built successfully: ${pagePath}`);
      return true;
    } catch (error) {
      console.error(`❌ Failed to build page ${pagePath}:`, error.message);
      throw new Error(`Failed to build page: ${pagePath}`);
    }
  }

  // Process HTML page (no build needed)
  processHtmlPage(pagePath, pageFiles, outputDir) {
    const pageInfo = this.getPageInfo(pagePath);
    const destPageDir = path.resolve(outputDir, pageInfo.dir);

    fs.mkdirSync(destPageDir, { recursive: true });
    console.log(`Created directory: ${destPageDir}`);

    // Copy HTML file with title and function injection
    const destFile = path.resolve(destPageDir, `${pageInfo.name}.html`);
    let htmlContent = fs.readFileSync(pageFiles.main.path, "utf-8");

    htmlContent = this.injectPageTitle(htmlContent, pageFiles.json);
    htmlContent = this.injectPageFunctions(htmlContent, pageFiles.js);

    fs.writeFileSync(destFile, htmlContent);
    console.log(`Copying HTML: ${pageFiles.main.path} → ${destFile}`);

    // Copy assets
    this.copyPageAssets(pageFiles, destPageDir, pageInfo.name);
  }

  // Process SPA page (Vue/React - needs build)
  async processSpaPage(pagePath, pageType, pageFiles, _rootDir, outputDir) {
    const buildDir = this.createBuildDirectory(pagePath);

    try {
      this.copyUserFiles(pageFiles, buildDir);
      const functions = this.extractFunctions(pageFiles.js.path);
      await this.generateFromTemplate(pageType, buildDir, functions, pageFiles);
      await this.runBuild(buildDir, pageType);
      this.copyBuildResult(buildDir, outputDir, pagePath, pageFiles);
    } finally {
      if (this.options.cleanup) {
        this.cleanup(buildDir);
      }
    }
  }

  // Create temporary build directory
  createBuildDirectory(pagePath) {
    const buildId = pagePath
      .replace(/[\/\\]/g, "-")
      .replace(/\.(html|vue|tsx)$/, "");
    const buildDir = path.resolve(this.options.buildDir, buildId);

    if (fs.existsSync(buildDir)) {
      fs.rmSync(buildDir, { recursive: true, force: true });
    }

    fs.mkdirSync(buildDir, { recursive: true });
    return buildDir;
  }

  // Copy user files to build directory
  copyUserFiles(pageFiles, buildDir) {
    if (pageFiles.main.exists) {
      const destFile = path.join(
        buildDir,
        "App" + path.extname(pageFiles.main.path),
      );
      fs.copyFileSync(pageFiles.main.path, destFile);
    }
  }

  // Extract functions from JS file
  extractFunctions(jsFilePath) {
    if (!jsFilePath || !fs.existsSync(jsFilePath)) {
      return [];
    }

    const allFunctions = extractPageFunctions(jsFilePath);
    const filteredFunctions = filterPageFunctions(allFunctions);
    console.log(
      `Extracted ${filteredFunctions.length} functions from ${jsFilePath}:`,
      filteredFunctions.map((f) => f.name),
    );
    return filteredFunctions;
  }

  // Generate build files from templates
  async generateFromTemplate(pageType, buildDir, functions, pageFiles) {
    if (pageType === PAGE_TYPES.VUE) {
      this.processVueTemplate(buildDir, functions, pageFiles);
    }
    // React support can be added later
  }

  // Process Vue template
  processVueTemplate(buildDir, functions, _pageFiles) {
    const templateDir = path.resolve(__dirname, "../templates/vue");

    // Copy template files
    const templateFiles = ["package.json", "vite.config.js", "index.html"];
    templateFiles.forEach((file) => {
      fs.copyFileSync(path.join(templateDir, file), path.join(buildDir, file));
    });

    // Generate main.js with function injection
    const mainJsTemplate = fs.readFileSync(
      path.join(templateDir, "main.js"),
      "utf-8",
    );
    const functionInjection = this.generateFunctionInjection(functions);

    const mainJsContent = mainJsTemplate.replace(
      /\/\*\s*\{\{PAGE_FUNCTIONS\}\}\s*\*\//,
      functionInjection,
    );

    fs.writeFileSync(path.join(buildDir, "main.js"), mainJsContent);
  }

  // Generate function injection code
  generateFunctionInjection(functions) {
    if (!functions || functions.length === 0) {
      return "// No page functions to inject";
    }

    const functionNames = functions.map((f) => f.name);
    return generatePageFunctionScript(functionNames);
  }

  // Run build command
  async runBuild(buildDir, _pageType) {
    console.log("Installing dependencies...");
    execSync("npm install", { cwd: buildDir, stdio: "inherit" });

    console.log("Running build...");
    execSync("npm run build", { cwd: buildDir, stdio: "inherit" });
  }

  // Copy build result to output directory
  copyBuildResult(buildDir, outputDir, pagePath, pageFiles) {
    const pageInfo = this.getPageInfo(pagePath);
    const destPageDir = path.resolve(outputDir, pageInfo.dir);

    fs.mkdirSync(destPageDir, { recursive: true });
    console.log(`Created directory: ${destPageDir}`);

    if (pageInfo.type === PAGE_TYPES.HTML) {
      // Already handled in processHtmlPage
      return;
    }

    // Copy SPA build result
    const distDir = path.join(buildDir, "dist");
    if (fs.existsSync(distDir)) {
      const indexHtml = path.join(distDir, "index.html");
      if (fs.existsSync(indexHtml)) {
        const destFile = path.resolve(
          destPageDir,
          `${pageInfo.name}${pageInfo.extension}`,
        );
        console.log(
          `Processing ${pageInfo.type.toUpperCase()}: ${indexHtml} → ${destFile}`,
        );

        let htmlContent = fs.readFileSync(indexHtml, "utf-8");
        // Inline assets manually since Vite plugin doesn't work reliably
        htmlContent = this.inlineAssets(htmlContent, distDir);
        htmlContent = this.injectPageTitle(htmlContent, pageFiles.json);
        htmlContent = this.injectPageFunctions(htmlContent, pageFiles.js);

        fs.writeFileSync(destFile, htmlContent);
      }
    }

    this.copyPageAssets(pageFiles, destPageDir, pageInfo.name);
  }

  // Inline CSS and JS assets into HTML
  inlineAssets(htmlContent, distDir) {
    // Inline CSS
    htmlContent = htmlContent.replace(
      /<link[^>]+href="([^"]+\.css)"[^>]*>/g,
      (match, cssFile) => {
        const cssPath = path.join(distDir, cssFile);
        if (fs.existsSync(cssPath)) {
          const cssContent = fs.readFileSync(cssPath, "utf-8");
          return `<style>${cssContent}</style>`;
        }
        return match;
      },
    );

    // Inline JS
    htmlContent = htmlContent.replace(
      /<script[^>]+src="([^"]+\.js)"[^>]*><\/script>/g,
      (match, jsFile) => {
        const jsPath = path.join(distDir, jsFile);
        if (fs.existsSync(jsPath)) {
          const jsContent = fs.readFileSync(jsPath, "utf-8");
          return `<script type="module">${jsContent}</script>`;
        }
        return match;
      },
    );

    return htmlContent;
  }

  // Inject page title from JSON config
  injectPageTitle(htmlContent, jsonFile) {
    if (jsonFile.exists) {
      try {
        const config = JSON.parse(fs.readFileSync(jsonFile.path, "utf-8"));
        const title = config.navigationBarTitleText || "LingXia Page";

        if (htmlContent.includes("<title>")) {
          htmlContent = htmlContent.replace(
            /<title>.*?<\/title>/i,
            `<title>${title}</title>`,
          );
        } else {
          htmlContent = htmlContent.replace(
            /<head>/i,
            `<head>\n  <title>${title}</title>`,
          );
        }
      } catch (error) {
        console.warn(`⚠️  Failed to parse JSON config: ${jsonFile.path}`);
      }
    }
    return htmlContent;
  }

  // Inject page functions into HTML
  injectPageFunctions(htmlContent, jsFile) {
    if (!jsFile.exists) {
      return htmlContent;
    }

    const functions = this.extractFunctions(jsFile.path);
    if (functions.length === 0) {
      return htmlContent;
    }

    const functionNames = functions.map((f) => f.name);
    const functionScript = `
    <script>
      ${generatePageFunctionScript(functionNames)}
    </script>`;

    return htmlContent.replace("</head>", `${functionScript}\n</head>`);
  }

  // Copy page assets (CSS, JSON)
  copyPageAssets(pageFiles, destPageDir, pageName) {
    if (pageFiles.css.exists) {
      const destCssFile = path.resolve(destPageDir, `${pageName}.css`);
      fs.copyFileSync(pageFiles.css.path, destCssFile);
      console.log(`Copied CSS: ${destCssFile}`);
    }

    if (pageFiles.json.exists) {
      const destJsonFile = path.resolve(destPageDir, `${pageName}.json`);
      fs.copyFileSync(pageFiles.json.path, destJsonFile);
      console.log(`Copied JSON: ${destJsonFile}`);
    }
  }

  // Get page info
  getPageInfo(pagePath) {
    const ext = path.extname(pagePath);
    const type = detectPageType(pagePath);
    const basePath = pagePath.replace(/\.(html|vue|tsx)$/, "");

    return {
      path: pagePath,
      type: type,
      extension: ext,
      dir: path.dirname(basePath),
      name: path.basename(basePath),
    };
  }

  // Clean up temporary directory
  cleanup(buildDir) {
    if (fs.existsSync(buildDir)) {
      fs.rmSync(buildDir, { recursive: true, force: true });
    }
  }
}
