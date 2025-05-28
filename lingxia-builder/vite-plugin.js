/**
 * LingXia MiniApp Builder - Vite Plugin
 *
 * Official build tool for LingXia MiniApp development.
 * Combines individual page files into optimized packages.
 *
 * Features:
 * - Unified logic.js generation
 * - Asset bundling (images, styles, layouts)
 * - JSON Validation: Validates all JSON files for syntax errors before building
 * - Code minification and optimization
 * - ZIP package generation
 */

import fs from "fs";
import path from "path";
import { minify } from "terser";
import archiver from "archiver";

/**
 * Validate JSON file syntax
 * @param {string} filePath - Path to JSON file
 * @param {string} content - JSON content to validate
 * @returns {Object} Validation result with isValid and error properties
 */
function validateJsonSyntax(filePath, content) {
  try {
    JSON.parse(content);
    return { isValid: true, error: null };
  } catch (error) {
    return {
      isValid: false,
      error: {
        message: error.message,
        line: getErrorLine(content, error),
        column: getErrorColumn(error),
      },
    };
  }
}

/**
 * Extract line number from JSON parse error
 * @param {string} content - JSON content
 * @param {Error} error - JSON parse error
 * @returns {number} Line number where error occurred
 */
function getErrorLine(content, error) {
  // Try to extract line number from error message
  const lineMatch = error.message.match(/line (\d+)/i);
  if (lineMatch) {
    return parseInt(lineMatch[1]);
  }

  // Try to extract position and calculate line
  const posMatch = error.message.match(/position (\d+)/i);
  if (posMatch) {
    const position = parseInt(posMatch[1]);
    const lines = content.substring(0, position).split("\n");
    return lines.length;
  }

  return 1;
}

/**
 * Extract column number from JSON parse error
 * @param {Error} error - JSON parse error
 * @returns {number} Column number where error occurred
 */
function getErrorColumn(error) {
  const columnMatch = error.message.match(/column (\d+)/i);
  if (columnMatch) {
    return parseInt(columnMatch[1]);
  }
  return 1;
}

/**
 * Validate all JSON files in the project
 * @param {string} rootDir - Root directory path
 * @param {Array<string>} pages - List of page paths
 * @returns {Array} Array of validation errors
 */
function validateAllJsonFiles(rootDir, pages) {
  const errors = [];
  const jsonFiles = new Set();

  // Add main configuration files
  const mainConfigFiles = ["app.json", "project.config.json"];

  for (const configFile of mainConfigFiles) {
    const filePath = path.resolve(rootDir, configFile);
    if (fs.existsSync(filePath)) {
      jsonFiles.add(filePath);
    }
  }

  // Add page JSON files
  for (const pagePath of pages) {
    const jsonPath = pagePath.replace(/\.html$/, ".json");
    const jsonFile = path.resolve(rootDir, jsonPath);
    if (fs.existsSync(jsonFile)) {
      jsonFiles.add(jsonFile);
    }
  }

  // Validate each JSON file
  for (const jsonFile of jsonFiles) {
    try {
      const content = fs.readFileSync(jsonFile, "utf-8");
      const validation = validateJsonSyntax(jsonFile, content);

      if (!validation.isValid) {
        const relativePath = path.relative(rootDir, jsonFile);
        errors.push({
          file: relativePath,
          error: validation.error,
        });
      }
    } catch (readError) {
      const relativePath = path.relative(rootDir, jsonFile);
      errors.push({
        file: relativePath,
        error: {
          message: `Failed to read file: ${readError.message}`,
          line: 1,
          column: 1,
        },
      });
    }
  }

  return errors;
}

/**
 * Transform Page() call to include page path parameter
 * @param {string} pageJs - Original page JavaScript code
 * @param {string} pagePath - Page path identifier
 * @returns {string} Transformed JavaScript code
 */
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

/**
 * Generate unified logic.js content
 * @param {Array<string>} pages - List of page paths
 * @param {string} rootDir - Root directory path
 * @param {Object} options - Generation options
 * @returns {string} Generated logic.js content
 */
async function generateLogicJs(pages, rootDir, options = {}) {
  const { minifyCode = false, removeComments = false } = options;

  let logicJs = "";
  let allImports = new Set();

  if (!removeComments) {
    const timestamp = new Date().toString();
    logicJs = `// Generated by LingXia MiniApp Builder
// Generated at: ${timestamp}
// Pages: ${pages.length}

`;
  }

  // Collect and resolve all dependencies (excluding page files)
  const resolvedDependencies = new Map();
  const processedFiles = new Set();
  const pageFiles = new Set();

  // Mark page files
  for (const pagePath of pages) {
    const jsPath = pagePath.replace(/\.html$/, ".js");
    const jsFile = path.resolve(rootDir, jsPath);
    pageFiles.add(jsFile);
  }

  // Recursively resolve dependencies
  function resolveDependencies(filePath, isPageFile = false) {
    if (processedFiles.has(filePath)) return;
    processedFiles.add(filePath);

    if (!fs.existsSync(filePath)) {
      console.warn(`⚠️  Dependency not found: ${filePath}`);
      return;
    }

    const content = fs.readFileSync(filePath, "utf-8");
    const importMatches = content.match(
      /^import\s+.*?from\s+['"]([^'"]+)['"];?\s*$/gm,
    );

    if (importMatches) {
      for (const importStatement of importMatches) {
        const match = importStatement.match(/from\s+['"]([^'"]+)['"]/);
        if (match) {
          const importPath = match[1];
          let resolvedPath;

          if (importPath.startsWith("./") || importPath.startsWith("../")) {
            // Relative import
            resolvedPath = path.resolve(path.dirname(filePath), importPath);
            if (!resolvedPath.endsWith(".js")) {
              resolvedPath += ".js";
            }
          } else {
            // Absolute import (relative to project root)
            resolvedPath = path.resolve(rootDir, importPath);
            if (!resolvedPath.endsWith(".js")) {
              resolvedPath += ".js";
            }
          }

          // Recursively resolve nested dependencies (but not page files)
          if (!pageFiles.has(resolvedPath)) {
            resolveDependencies(resolvedPath, false);
          }
        }
      }
    }

    // Only store dependencies, not page files
    if (!isPageFile) {
      // Store the cleaned content (without import/export statements)
      let cleanContent = content.replace(
        /^import\s+.*?from\s+['"][^'"]+['"];?\s*$/gm,
        "",
      );

      // Convert export statements to global assignments
      cleanContent = cleanContent.replace(
        /^export\s+function\s+(\w+)/gm,
        "globalThis.$1 = function $1",
      );
      cleanContent = cleanContent.replace(
        /^export\s+const\s+(\w+)/gm,
        "globalThis.$1",
      );
      cleanContent = cleanContent.replace(
        /^export\s+let\s+(\w+)/gm,
        "globalThis.$1",
      );
      cleanContent = cleanContent.replace(
        /^export\s+var\s+(\w+)/gm,
        "globalThis.$1",
      );
      cleanContent = cleanContent.replace(
        /^export\s+\{([^}]+)\}/gm,
        (match, exports) => {
          const exportList = exports.split(",").map((e) => e.trim());
          return exportList
            .map((exp) => `globalThis.${exp} = ${exp};`)
            .join("\n");
        },
      );

      if (cleanContent.trim()) {
        resolvedDependencies.set(filePath, cleanContent);
      }
    }
  }

  // Collect dependencies from all pages
  for (const pagePath of pages) {
    const jsPath = pagePath.replace(/\.html$/, ".js");
    const jsFile = path.resolve(rootDir, jsPath);
    if (fs.existsSync(jsFile)) {
      resolveDependencies(jsFile, true); // Mark as page file
    }
  }

  // Add resolved dependencies to logic.js
  if (resolvedDependencies.size > 0) {
    if (!removeComments) {
      logicJs += `// === Dependencies ===\n`;
    }

    // Add dependencies in dependency order (dependencies first, then dependents)
    const addedDeps = new Set();

    function addDependency(filePath) {
      if (addedDeps.has(filePath) || !resolvedDependencies.has(filePath))
        return;

      const content = fs.readFileSync(filePath, "utf-8");
      const importMatches = content.match(
        /^import\s+.*?from\s+['"]([^'"]+)['"];?\s*$/gm,
      );

      // Add dependencies first
      if (importMatches) {
        for (const importStatement of importMatches) {
          const match = importStatement.match(/from\s+['"]([^'"]+)['"]/);
          if (match) {
            const importPath = match[1];
            let resolvedPath;

            if (importPath.startsWith("./") || importPath.startsWith("../")) {
              resolvedPath = path.resolve(path.dirname(filePath), importPath);
              if (!resolvedPath.endsWith(".js")) {
                resolvedPath += ".js";
              }
            } else {
              resolvedPath = path.resolve(rootDir, importPath);
              if (!resolvedPath.endsWith(".js")) {
                resolvedPath += ".js";
              }
            }

            addDependency(resolvedPath);
          }
        }
      }

      // Then add this dependency
      if (resolvedDependencies.has(filePath)) {
        addedDeps.add(filePath);
        const relativePath = path.relative(rootDir, filePath);

        if (!removeComments) {
          logicJs += `\n// === ${relativePath} ===\n`;
        }
        logicJs += `(function() {\n${resolvedDependencies.get(filePath)}\n})();\n`;
      }
    }

    // Add all dependencies
    for (const filePath of resolvedDependencies.keys()) {
      addDependency(filePath);
    }

    logicJs += "\n";
  }

  // Include app.js content first (App should be executed before Page registrations)
  const appJsPath = path.resolve(rootDir, "app.js");
  if (fs.existsSync(appJsPath)) {
    const appJs = fs.readFileSync(appJsPath, "utf-8");
    if (!removeComments) {
      logicJs += `// === App Logic ===\n`;
    }
    logicJs += `${appJs}\n\n`;
  }

  if (!removeComments) {
    logicJs += `// === Page Registrations ===\n`;
  }

  logicJs += `(function() {
  console.log('🚀 LingXia MiniApp loaded with ${pages.length} pages');
`;

  let processedPages = 0;

  for (const pagePath of pages) {
    // Convert page path to JS file path (remove .html, add .js)
    const jsPath = pagePath.replace(/\.html$/, ".js");
    const jsFile = path.resolve(rootDir, jsPath);

    if (fs.existsSync(jsFile)) {
      try {
        let pageJs = fs.readFileSync(jsFile, "utf-8");

        // Remove import statements (they're already at the top level)
        pageJs = pageJs.replace(
          /^import\s+.*?from\s+['"][^'"]+['"];?\s*$/gm,
          "",
        );

        // Use original pagePath (with .html) for Page() registration
        const transformedJs = transformPageCall(pageJs, pagePath);

        if (!removeComments) {
          logicJs += `
  // === ${pagePath} ===`;
        }

        logicJs += `
  (function() {
    ${transformedJs}
  })();
`;
        processedPages++;
      } catch (error) {
        console.error(`❌ Error processing ${pagePath}:`, error.message);
      }
    } else {
      console.warn(`⚠️  Page file not found: ${jsFile}`);
    }
  }

  // Close the Page Registrations function
  logicJs += `
})();
`;

  if (minifyCode || removeComments) {
    try {
      const result = await minify(logicJs, {
        compress: minifyCode
          ? {
              drop_console: false,
              drop_debugger: true,
            }
          : false,
        mangle: minifyCode,
        format: {
          comments: !removeComments,
        },
      });
      logicJs = result.code;
    } catch (error) {
      console.warn(
        "⚠️  Code processing failed, using original code:",
        error.message,
      );
    }
  }

  console.log(
    `✨ Generated logic.js with ${processedPages}/${pages.length} pages`,
  );
  return logicJs;
}

/**
 * Process and copy global CSS file to build directory
 * Only processes plain CSS files that WebView can understand
 * @param {string} rootDir - Root directory
 * @param {string} buildDir - Build output directory
 */
async function processGlobalCSS(rootDir, buildDir) {
  // Only look for plain CSS files that WebView can handle
  const globalCSSFiles = ["app.css"];

  for (const cssFile of globalCSSFiles) {
    const srcFile = path.resolve(rootDir, cssFile);
    if (fs.existsSync(srcFile)) {
      try {
        const destFile = path.resolve(buildDir, "app.css");
        fs.copyFileSync(srcFile, destFile);
        console.log(`🎨 Copied global CSS: ${cssFile} → app.css`);
        return true;
      } catch (error) {
        console.warn(
          `⚠️  Failed to process CSS file ${cssFile}:`,
          error.message,
        );
      }
    }
  }

  console.log(
    `ℹ️  No global CSS file found (checked: ${globalCSSFiles.join(", ")})`,
  );
  return false;
}

/**
 * Copy assets to build directory
 * @param {string} rootDir - Root directory
 * @param {string} buildDir - Build output directory
 * @param {Array<string>} assetDirs - Asset directories to copy
 */
function copyAssets(rootDir, buildDir, assetDirs = ["images"]) {
  for (const assetDir of assetDirs) {
    const srcDir = path.resolve(rootDir, assetDir);
    const destDir = path.resolve(buildDir, assetDir);

    if (fs.existsSync(srcDir)) {
      fs.mkdirSync(destDir, { recursive: true });
      copyDirectory(srcDir, destDir);
      console.log(`📁 Copied assets: ${assetDir}`);
    }
  }
}

/**
 * Copy directory recursively
 * @param {string} src - Source directory
 * @param {string} dest - Destination directory
 */
function copyDirectory(src, dest) {
  const entries = fs.readdirSync(src, { withFileTypes: true });

  for (const entry of entries) {
    const srcPath = path.join(src, entry.name);
    const destPath = path.join(dest, entry.name);

    if (entry.isDirectory()) {
      fs.mkdirSync(destPath, { recursive: true });
      copyDirectory(srcPath, destPath);
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

/**
 * Copy page assets (HTML, CSS, JSON)
 * @param {Array<string>} pages - List of page paths
 * @param {string} rootDir - Root directory
 * @param {string} buildDir - Build output directory
 */
function copyPageAssets(pages, rootDir, buildDir) {
  for (const pagePath of pages) {
    // Remove .html extension to get base path
    const basePath = pagePath.replace(/\.html$/, "");
    const pageDir = path.dirname(basePath);
    const pageName = path.basename(basePath);

    const srcPageDir = path.resolve(rootDir, pageDir);
    const destPageDir = path.resolve(buildDir, pageDir);

    fs.mkdirSync(destPageDir, { recursive: true });

    // Copy HTML, CSS, JSON files
    const extensions = [".html", ".css", ".json"];
    for (const ext of extensions) {
      const srcFile = path.resolve(srcPageDir, `${pageName}${ext}`);
      const destFile = path.resolve(destPageDir, `${pageName}${ext}`);

      if (fs.existsSync(srcFile)) {
        fs.copyFileSync(srcFile, destFile);
      }
    }
  }

  console.log(`📄 Copied page assets for ${pages.length} pages`);
}

/**
 * Get file type for build processing
 * @param {string} filePath - File path
 * @returns {string} File type
 */
function getFileType(filePath) {
  const ext = path.extname(filePath).toLowerCase();
  switch (ext) {
    case ".js":
      return filePath.endsWith("app.js") ? "app" : "page";
    case ".html":
      return "layout";
    case ".css":
      return "style";
    case ".json":
      return filePath.endsWith("app.json") ? "config" : "page-config";
    case ".png":
    case ".jpg":
    case ".jpeg":
    case ".gif":
    case ".svg":
    case ".webp":
      return "image";
    default:
      return "asset";
  }
}

/**
 * Create ZIP package
 * @param {string} buildDir - Build directory
 * @param {string} outputPath - Output ZIP path
 * @returns {Promise<void>}
 */
function createZipPackage(buildDir, outputPath) {
  return new Promise((resolve, reject) => {
    // Check if build directory exists and has files
    if (!fs.existsSync(buildDir)) {
      reject(new Error(`Build directory does not exist: ${buildDir}`));
      return;
    }

    const files = fs.readdirSync(buildDir);
    if (files.length === 0) {
      reject(new Error(`Build directory is empty: ${buildDir}`));
      return;
    }

    const output = fs.createWriteStream(outputPath);
    const archive = archiver("zip", { zlib: { level: 9 } });

    output.on("close", () => {
      console.log(
        `📦 Package created: ${outputPath} (${archive.pointer()} bytes)`,
      );
      resolve();
    });

    output.on("error", reject);
    archive.on("error", reject);

    archive.pipe(output);
    archive.directory(buildDir, false);
    archive.finalize();
  });
}

/**
 * LingXia MiniApp Builder Vite Plugin
 * @param {Object} options - Plugin options
 * @returns {Object} Vite plugin configuration
 */
export function LingXiaMiniAppBuilder(options = {}) {
  const {
    configFile = "app.json",
    projectConfigFile = "project.config.json",
    outputFile = "logic.js",
    buildDir = "dist",
    assetDirs = ["images"],
    minifyCode = false,
    removeComments = false,
    createPackage = false,
    packageName = null, // Will use appId if not specified
    appId = null, // MiniApp identifier (fallback)
    targetDir = null, // Target directory for host app assets
    copyToTarget = false, // Whether to copy to target directory
  } = options;

  let rootDir = "";

  let hasGenerated = false;

  return {
    name: "lingxia-miniapp-builder",

    configResolved(config) {
      rootDir = config.root || process.cwd();
    },

    writeBundle() {
      if (!hasGenerated) {
        hasGenerated = true;
        generatePackage();
      }
    },
  };

  async function generatePackage() {
    try {
      console.log("🚀 LingXia MiniApp Builder starting...");

      // Read configuration
      const configPath = path.resolve(rootDir, configFile);
      if (!fs.existsSync(configPath)) {
        throw new Error(`Configuration file not found: ${configPath}`);
      }

      const appConfig = JSON.parse(fs.readFileSync(configPath, "utf-8"));

      if (!appConfig.pages || !Array.isArray(appConfig.pages)) {
        throw new Error("Invalid app.json: pages array not found");
      }

      // Get appId from project config or app config (required)
      let finalAppId = appId;

      // Try to read from project.config.json first (WeChat standard)
      const projectConfigPath = path.resolve(rootDir, projectConfigFile);
      if (fs.existsSync(projectConfigPath)) {
        try {
          const projectConfig = JSON.parse(
            fs.readFileSync(projectConfigPath, "utf-8"),
          );
          finalAppId = finalAppId || projectConfig.appid;
          console.log(`📋 Found project config: ${projectConfigFile}`);
        } catch (error) {
          console.warn(`⚠️  Failed to read project config: ${error.message}`);
        }
      }

      // Fallback to app.json
      finalAppId = finalAppId || appConfig.appId;

      if (!finalAppId) {
        throw new Error(
          "appId is required. Please set it in project.config.json (appid field) or app.json (appId field) or plugin options.",
        );
      }
      console.log(`📱 App ID: ${finalAppId}`);

      // Extract page paths (keep original format for Page() registration)
      const pages = appConfig.pages;
      console.log(`📄 Found ${pages.length} pages:`, pages);

      // Validate all JSON files
      console.log("🔍 Validating JSON files...");
      const jsonErrors = validateAllJsonFiles(rootDir, pages);

      if (jsonErrors.length > 0) {
        console.error("❌ JSON validation failed:");
        for (const error of jsonErrors) {
          console.error(`  📄 ${error.file}:`);
          console.error(
            `     Line ${error.error.line}, Column ${error.error.column}: ${error.error.message}`,
          );
        }
        throw new Error(
          `Found ${jsonErrors.length} JSON syntax error(s). Please fix them before building.`,
        );
      }
      console.log("✅ All JSON files are valid");

      // Create build directory
      const fullBuildDir = path.resolve(rootDir, buildDir);
      fs.mkdirSync(fullBuildDir, { recursive: true });

      // Copy app.json only (app.js is merged into logic.js)
      fs.copyFileSync(configPath, path.resolve(fullBuildDir, "app.json"));

      // Copy project.config.json if exists
      if (fs.existsSync(projectConfigPath)) {
        fs.copyFileSync(
          projectConfigPath,
          path.resolve(fullBuildDir, "project.config.json"),
        );
      }

      // Generate logic.js (includes app.js content)
      const logicContent = await generateLogicJs(pages, rootDir, {
        minifyCode,
        removeComments,
      });

      const logicPath = path.resolve(fullBuildDir, outputFile);
      fs.writeFileSync(logicPath, logicContent, "utf-8");

      // Process and copy global CSS file if exists
      await processGlobalCSS(rootDir, fullBuildDir);

      // Copy page assets
      copyPageAssets(pages, rootDir, fullBuildDir);

      // Copy other assets
      copyAssets(rootDir, fullBuildDir, assetDirs);

      // Clean up unwanted files generated by Vite
      const unwantedFiles = [
        "entry.js",
        "main.js",
        "index.js",
        "temp.js",
        "app.js",
      ];
      for (const file of unwantedFiles) {
        const filePath = path.resolve(fullBuildDir, file);
        if (fs.existsSync(filePath)) {
          fs.unlinkSync(filePath);
          console.log(`🗑️  Removed unwanted file: ${file}`);
        }
      }

      // Copy to target directory if specified
      if (copyToTarget && targetDir) {
        const finalTargetDir = path.resolve(targetDir, finalAppId);
        console.log(`📁 Copying to target directory: ${finalTargetDir}`);
        fs.mkdirSync(finalTargetDir, { recursive: true });
        copyDirectory(fullBuildDir, finalTargetDir);
        console.log(`✅ Copied to target: ${finalTargetDir}`);
      }

      // Create ZIP package if requested
      if (createPackage) {
        const finalPackageName = packageName || `${finalAppId}.zip`;
        const packagePath = path.resolve(rootDir, finalPackageName);
        try {
          await createZipPackage(fullBuildDir, packagePath);
        } catch (error) {
          console.warn("⚠️  Failed to create ZIP package:", error.message);
        }
      }

      console.log("✅ LingXia MiniApp build completed successfully");
    } catch (error) {
      console.error("❌ LingXia MiniApp Builder failed:", error.message);
      throw error;
    }
  }
}
