// LingXia MiniApp Builder - Vite Plugin
import fs from "fs";
import path from "path";
import archiver from "archiver";

import { parseAppConfig } from "./src/core/app-config.js";
import { PageBuilder } from "./src/core/page-builder.js";

export function lingxiaPlugin(options = {}) {
  const {
    appConfig = "./app.json",
    outputDir = "dist",
    buildDir = ".lingxia-build",
    cleanup = true,
    createPackage = false,
    minifyCode = false,
  } = options;

  let config;
  let rootDir;

  return {
    name: "lingxia",

    configResolved(resolvedConfig) {
      config = resolvedConfig;
      rootDir = config.root;
      console.log("🚀 LingXia MiniApp Builder starting...");
    },

    async writeBundle() {
      try {
        const appConfigPath = path.resolve(rootDir, appConfig);
        const appInfo = parseAppConfig(appConfigPath);

        console.log(`Found project config: ${appConfig}`);
        console.log(`App ID: ${appInfo.lxAppId}`);
        console.log(
          `Found ${appInfo.pages.length} pages: ${JSON.stringify(appInfo.pages)}`,
        );

        const fullOutputDir = path.resolve(rootDir, outputDir);

        // Build pages
        await buildPages(
          appInfo.pages,
          fullOutputDir,
          rootDir,
          buildDir,
          cleanup,
        );

        // Generate logic.js
        await generateLogicJS(
          appInfo.pages,
          fullOutputDir,
          rootDir,
          minifyCode,
        );

        // Copy static assets
        await copyStaticAssets(fullOutputDir, rootDir);

        // Copy app config
        await copyAppConfig(appConfigPath, fullOutputDir);

        // Clean temp files
        const tempBuildDir = path.resolve(rootDir, "temp-build");
        if (fs.existsSync(tempBuildDir)) {
          fs.rmSync(tempBuildDir, { recursive: true, force: true });
          console.log("Removed temp-build directory");
        }

        // Create package
        if (createPackage) {
          await createPackageFile(appInfo, fullOutputDir);
        }

        console.log("LingXia MiniApp build completed successfully");
      } catch (error) {
        console.error("❌ LingXia build failed:", error.message);
        throw error;
      }
    },
  };
}

// Build all pages
async function buildPages(pages, outputDir, rootDir, buildDir, cleanup) {
  console.log("Building pages...");

  const pageBuilder = new PageBuilder({ buildDir, cleanup });

  const buildPromises = pages.map(async (pagePath) => {
    try {
      const success = await pageBuilder.buildPage(pagePath, rootDir, outputDir);
      if (!success) {
        throw new Error(`Failed to build page: ${pagePath}`);
      }
    } catch (error) {
      console.error(`❌ Error building page ${pagePath}:`, error.message);
      throw error;
    }
  });

  await Promise.all(buildPromises);
  console.log("All pages built successfully");
}

// Generate logic.js (merge app.js + page JS + common JS)
async function generateLogicJS(pages, outputDir, rootDir, minifyCode = false) {
  console.log("Generating logic.js...");

  let logicCode = "";

  // Add app.js content
  const appJsPath = path.resolve(rootDir, "app.js");
  if (fs.existsSync(appJsPath)) {
    const appContent = fs.readFileSync(appJsPath, "utf-8");
    logicCode += `// === App Entry ===\n${appContent}\n\n`;
  }

  // Add common JS files
  const commonDir = path.resolve(rootDir, "common");
  if (fs.existsSync(commonDir)) {
    logicCode += await processCommonJS(commonDir);
  }

  // Add page JS functions
  for (const pagePath of pages) {
    const pageJS = await processPageJS(pagePath, rootDir);
    if (pageJS) {
      const pageJSWithPath = transformPageCall(pageJS, pagePath);
      logicCode += `\n// === Page: ${pagePath} ===\n${pageJSWithPath}\n`;
    }
  }

  // Minify code if requested (includes removing comments)
  if (minifyCode || process.env.NODE_ENV === "production") {
    logicCode = await minifyJSCode(logicCode);
  }

  // Write logic.js
  const logicPath = path.join(outputDir, "logic.js");
  fs.writeFileSync(logicPath, logicCode, "utf-8");

  console.log(
    `Generated logic.js (${(logicCode.length / 1024).toFixed(2)} KB)`,
  );
}

// Process common JS files
async function processCommonJS(commonDir) {
  let commonCode = "// === Common Utilities ===\n";

  const files = fs.readdirSync(commonDir);
  for (const file of files) {
    if (file.endsWith(".js")) {
      const filePath = path.join(commonDir, file);
      const content = fs.readFileSync(filePath, "utf-8");
      const processedContent = convertESModuleToGlobal(content);
      commonCode += `\n// --- ${file} ---\n${processedContent}\n`;
    }
  }

  return commonCode;
}

// Transform Page() call to include page path parameter
function transformPageCall(pageJs, pagePath) {
  const lastPageIndex = pageJs.lastIndexOf("Page(");
  if (lastPageIndex === -1) {
    console.warn(`⚠️  No Page() call found in ${pagePath}`);
    return pageJs;
  }

  let braceCount = 0;
  let parenCount = 0;
  let inString = false;
  let stringChar = "";
  let i = lastPageIndex + 5; // Skip "Page("

  for (; i < pageJs.length; i++) {
    const char = pageJs[i];

    if (!inString) {
      if (char === '"' || char === "'" || char === "`") {
        inString = true;
        stringChar = char;
      } else if (char === "{") {
        braceCount++;
      } else if (char === "}") {
        braceCount--;
      } else if (char === "(") {
        parenCount++;
      } else if (char === ")") {
        if (parenCount === 0 && braceCount === 0) {
          break;
        }
        parenCount--;
      }
    } else {
      if (char === stringChar && pageJs[i - 1] !== "\\") {
        inString = false;
      }
    }
  }

  if (i < pageJs.length) {
    const beforePage = pageJs.substring(0, lastPageIndex);
    const pageCall = pageJs.substring(lastPageIndex, i + 1);
    const afterPage = pageJs.substring(i + 1);

    const newPageCall = pageCall
      .replace(/Page\s*\(\s*{/, "Page({")
      .replace(/}\s*\)$/, `}, "${pagePath}")`);

    return beforePage + newPageCall + afterPage;
  }

  return pageJs;
}

// Process single page JS file
async function processPageJS(pagePath, rootDir) {
  const pageDir = path.dirname(pagePath);
  const pageName = path.basename(pagePath, path.extname(pagePath));
  const jsPath = path.resolve(rootDir, pageDir, `${pageName}.js`);

  if (!fs.existsSync(jsPath)) {
    return null;
  }

  // Return original Page() code directly
  const content = fs.readFileSync(jsPath, "utf-8");
  return content;
}

// Convert ES Module to global functions
function convertESModuleToGlobal(code) {
  return code
    .replace(/export\s+function\s+/g, "function ")
    .replace(/export\s+const\s+(\w+)\s*=\s*/g, "const $1 = ")
    .replace(/export\s+\{[^}]+\}/g, ""); // Remove export { ... }
}

// Copy static assets
async function copyStaticAssets(outputDir, rootDir) {
  console.log("Copying static assets...");

  // Common static asset directories
  const staticDirs = ["images", "assets", "static", "public"];

  for (const dirName of staticDirs) {
    const staticDir = path.resolve(rootDir, dirName);
    if (fs.existsSync(staticDir)) {
      const destDir = path.join(outputDir, dirName);
      await copyDirectory(staticDir, destDir);
      console.log(`Copied ${dirName}/ directory`);
    }
  }

  // Copy app.css
  const appCss = path.resolve(rootDir, "app.css");
  if (fs.existsSync(appCss)) {
    fs.copyFileSync(appCss, path.join(outputDir, "app.css"));
    console.log("Copied app.css");
  }
}

// Copy app configuration
async function copyAppConfig(appConfigPath, outputDir) {
  const destPath = path.join(outputDir, "app.json");
  fs.copyFileSync(appConfigPath, destPath);
  console.log("Copied app.json");
}

// Create package file (.zip)
async function createPackageFile(appInfo, outputDir) {
  console.log("Creating package...");

  const packageName = `${appInfo.lxAppId || "miniapp"}.zip`;
  const packagePath = path.join(path.dirname(outputDir), packageName);

  return new Promise((resolve, reject) => {
    const output = fs.createWriteStream(packagePath);
    const archive = archiver("zip", { zlib: { level: 9 } });

    output.on("close", () => {
      console.log(
        `Package created: ${packageName} (${(archive.pointer() / 1024).toFixed(2)} KB)`,
      );
      resolve();
    });

    archive.on("error", reject);
    archive.pipe(output);

    // Add entire dist directory to archive
    archive.directory(outputDir, false);
    archive.finalize();
  });
}

// Minify JavaScript code (includes removing comments)
async function minifyJSCode(code) {
  try {
    // Try to use terser for proper minification
    const { minify } = await import("terser");
    const result = await minify(code, {
      compress: {
        drop_console: false, // Keep console.log for debugging
        drop_debugger: true,
        pure_funcs: [], // Don't remove any function calls
      },
      mangle: false, // Don't mangle variable names to keep readability
      format: {
        comments: false, // Remove all comments
        beautify: false, // Minify output
      },
    });
    return result.code || code;
  } catch (error) {
    console.warn(
      "⚠️  Code minification failed, using simple comment removal:",
      error.message,
    );
    // Fallback to simple comment removal
    return removeCommentsSimple(code);
  }
}

// Simple comment removal fallback
function removeCommentsSimple(code) {
  return code
    .replace(/\/\*[\s\S]*?\*\//g, "") // Remove block comments
    .replace(/\/\/.*$/gm, "") // Remove line comments
    .replace(/^\s*[\r\n]/gm, ""); // Remove empty lines
}

// Copy directory recursively
async function copyDirectory(src, dest) {
  if (!fs.existsSync(dest)) {
    fs.mkdirSync(dest, { recursive: true });
  }

  const files = fs.readdirSync(src);
  for (const file of files) {
    const srcPath = path.join(src, file);
    const destPath = path.join(dest, file);

    if (fs.statSync(srcPath).isDirectory()) {
      await copyDirectory(srcPath, destPath);
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

export default lingxiaPlugin;
