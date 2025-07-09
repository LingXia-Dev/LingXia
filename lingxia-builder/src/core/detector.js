import fs from "fs";
import path from "path";

// PAGE_TYPES moved to processors/index.js to avoid circular imports

// Detect page type from file extension
export function detectPageType(pagePath) {
  const ext = path.extname(pagePath).toLowerCase();

  switch (ext) {
    case ".vue":
      return "vue";
    case ".jsx":
    case ".tsx":
      return "react";
    case ".html":
      return "html";
    default:
      return "html";
  }
}

// Get page files (JS, JSON, CSS) for a page
export function getPageFiles(pagePath, rootDir) {
  const pageInfo = getPageBasePath(pagePath);
  const basePath = path.resolve(rootDir, pageInfo.dir, pageInfo.name);

  return {
    type: detectPageType(pagePath),
    main: {
      path: path.resolve(rootDir, pagePath),
      exists: fs.existsSync(path.resolve(rootDir, pagePath)),
    },
    js: {
      path: `${basePath}.js`,
      exists: fs.existsSync(`${basePath}.js`),
    },
    json: {
      path: `${basePath}.json`,
      exists: fs.existsSync(`${basePath}.json`),
    },
    css: {
      path: `${basePath}.css`,
      exists: fs.existsSync(`${basePath}.css`),
    },
  };
}

function getPageBasePath(pagePath) {
  const ext = path.extname(pagePath);
  const basePath = pagePath.replace(ext, "");

  return {
    dir: path.dirname(basePath),
    name: path.basename(basePath),
    extension: ext,
  };
}
