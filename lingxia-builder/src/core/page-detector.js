import fs from "fs";
import path from "path";

export const PAGE_TYPES = {
  VUE: "vue",
  REACT: "react",
  HTML: "html",
};

// Detect page type from file extension
export function detectPageType(pagePath) {
  const ext = path.extname(pagePath).toLowerCase();

  switch (ext) {
    case ".vue":
      return PAGE_TYPES.VUE;
    case ".jsx":
    case ".tsx":
      return PAGE_TYPES.REACT;
    case ".html":
      return PAGE_TYPES.HTML;
    default:
      throw new Error(`Unsupported page file extension: ${ext}`);
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
