import fs from "fs";
import path from "path";
import { execSync } from "child_process";
import { detectPageType, getPageFiles } from "./detector.js";
import { extractPageFunctions, filterPageFunctions } from "./extractor.js";
import { ProcessorFactory, PAGE_TYPES } from "./processors/index.js";

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
      console.log(`Building page: ${pagePath}`);

      const pageType = detectPageType(pagePath);
      const pageFiles = getPageFiles(pagePath, rootDir);

      if (pageType === PAGE_TYPES.HTML) {
        this.processHtmlPage(pagePath, pageFiles, rootDir, outputDir);
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
      console.error(`Stack trace:`, error.stack);
      throw new Error(`Failed to build page: ${pagePath}`);
    }
  }

  // Process HTML page (static)
  processHtmlPage(pagePath, pageFiles, rootDir, outputDir) {
    const pageInfo = this.getPageInfo(pagePath);
    const destPageDir = path.resolve(outputDir, pageInfo.dir);

    if (!fs.existsSync(destPageDir)) {
      fs.mkdirSync(destPageDir, { recursive: true });
      console.log(`Created directory: ${destPageDir}`);
    }

    const destFile = path.resolve(destPageDir, pageInfo.name + ".html");
    console.log(`Copying HTML: ${pageFiles.main.path} → ${destFile}`);

    // Read HTML content and inject page functions
    let htmlContent = fs.readFileSync(pageFiles.main.path, "utf-8");
    htmlContent = this.injectPageTitle(htmlContent, pageFiles.json);
    htmlContent = this.injectPageFunctions(htmlContent, pageFiles.js);

    fs.writeFileSync(destFile, htmlContent);
    this.copyPageAssets(pageFiles, destPageDir, pageInfo.name);
  }

  // Process SPA page (Vue/React - needs build)
  async processSpaPage(pagePath, pageType, pageFiles, rootDir, outputDir) {
    const buildDir = this.createBuildDirectory(pagePath);

    try {
      this.copyUserFiles(pageFiles, buildDir);
      const functions = this.extractFunctions(pageFiles.js.path);
      await this.generateFromTemplate(pageType, buildDir, functions, pageFiles);
      await this.runBuild(buildDir, pageType);
      this.copyBuiltPage(buildDir, pagePath, pageFiles, outputDir);
    } finally {
      if (this.options.cleanup && fs.existsSync(buildDir)) {
        // Keep build directory for debugging in development
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
    // Copy user files if they exist
    Object.entries(pageFiles).forEach(([type, file]) => {
      if (file.exists && type !== "main") {
        const destPath = path.join(buildDir, path.basename(file.path));
        fs.copyFileSync(file.path, destPath);
      }
    });
  }

  // Extract and filter functions
  extractFunctions(jsFilePath) {
    const allFunctions = extractPageFunctions(jsFilePath);
    const filteredFunctions = filterPageFunctions(allFunctions);

    if (filteredFunctions.length > 0) {
      const functionNames = filteredFunctions.map((f) => f.name);
      console.log(
        `Extracted ${filteredFunctions.length} functions from ${jsFilePath}: [${functionNames.map((name) => ` '${name}'`).join(",")} ]`,
      );
    }

    return filteredFunctions;
  }

  // Generate build files from templates
  async generateFromTemplate(pageType, buildDir, functions, pageFiles) {
    await ProcessorFactory.process(pageType, buildDir, functions, pageFiles);
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
  async runBuild(buildDir, pageType) {
    console.log("Installing dependencies...");
    execSync("npm install", { cwd: buildDir, stdio: "inherit" });

    console.log("Running build...");
    execSync("npm run build", { cwd: buildDir, stdio: "inherit" });
  }

  // Copy built page to output directory
  copyBuiltPage(buildDir, pagePath, pageFiles, outputDir) {
    const pageInfo = this.getPageInfo(pagePath);
    const destPageDir = path.resolve(outputDir, pageInfo.dir);

    if (!fs.existsSync(destPageDir)) {
      fs.mkdirSync(destPageDir, { recursive: true });
      console.log(`Created directory: ${destPageDir}`);
    }

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
        htmlContent = this.inlineAssets(htmlContent, distDir);
        htmlContent = this.injectPageTitle(htmlContent, pageFiles.json);
        htmlContent = this.injectPageFunctions(htmlContent, pageFiles.js);

        fs.writeFileSync(destFile, htmlContent);
      }
    }

    this.copyPageAssets(pageFiles, destPageDir, pageInfo.name);
  }

  // Copy page assets (CSS, JSON)
  copyPageAssets(pageFiles, destPageDir, pageName) {
    if (pageFiles.css.exists) {
      const destCssPath = path.join(destPageDir, `${pageName}.css`);
      fs.copyFileSync(pageFiles.css.path, destCssPath);
      console.log(`Copied CSS: ${destCssPath}`);
    }

    if (pageFiles.json.exists) {
      const destJsonPath = path.join(destPageDir, `${pageName}.json`);
      fs.copyFileSync(pageFiles.json.path, destJsonPath);
      console.log(`Copied JSON: ${destJsonPath}`);
    }
  }

  // Get page info from path
  getPageInfo(pagePath) {
    const ext = path.extname(pagePath);
    const basePath = pagePath.replace(ext, "");
    const pageType = detectPageType(pagePath);

    return {
      dir: path.dirname(basePath),
      name: path.basename(basePath),
      extension: ext,
      type: pageType,
    };
  }

  // Inline assets into HTML
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
    if (!jsonFile.exists) return htmlContent;

    try {
      const config = JSON.parse(fs.readFileSync(jsonFile.path, "utf-8"));
      if (config.navigationBarTitleText) {
        htmlContent = htmlContent.replace(
          /<title>.*?<\/title>/,
          `<title>${config.navigationBarTitleText}</title>`,
        );
      }
    } catch (error) {
      console.warn("Failed to parse page JSON config:", error.message);
    }

    return htmlContent;
  }

  // Inject page functions into HTML
  injectPageFunctions(htmlContent, jsFile) {
    if (!jsFile.exists) return htmlContent;

    const functions = this.extractFunctions(jsFile.path);
    const functionInjection = this.generateFunctionInjection(functions);

    return htmlContent.replace(
      "</head>",
      `<script>${functionInjection}</script></head>`,
    );
  }
}
