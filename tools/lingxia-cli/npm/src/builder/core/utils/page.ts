import type { FrameworkType, Page, PageFiles } from "../../types/index.js";
import { FileUtils } from "./file.js";
import fs from "fs";
import path from "path";

const fileUtils = new FileUtils();

/**
 * Get page title from configuration or default
 */
export function getPageTitle(page: Page, pageFiles: PageFiles): string {
  if (pageFiles.config.exists) {
    const config = fileUtils.readJsonFile(pageFiles.config.path);
    if (config?.navigationBarTitleText) {
      return config.navigationBarTitleText;
    }
  }

  // Fallback to page name
  const pageName = fileUtils.getBaseName(page.path);
  return pageName.charAt(0).toUpperCase() + pageName.slice(1);
}

/**
 * Detect page type from file extension
 */
export function detectPageType(filePath: string): "html" | "react" | "vue" {
  const ext = fileUtils.getExtension(filePath);

  switch (ext) {
    case "tsx":
    case "jsx":
      return "react";
    case "vue":
      return "vue";
    case "html":
    default:
      return "html";
  }
}

/**
 * View file extensions in priority order for each framework
 */
const VIEW_EXTENSIONS: Record<FrameworkType, string[]> = {
  react: ["tsx", "jsx"],
  vue: ["vue"],
};

/**
 * All supported view extensions for auto-detection
 */
const ALL_VIEW_EXTENSIONS = ["tsx", "jsx", "vue", "html"];

/**
 * Resolve page path to actual file path with extension.
 * If pagePath has no extension, auto-detect or use specified framework.
 *
 * @param projectPath - Project root directory
 * @param pagePath - Page path from lxapp.json (may or may not have extension)
 * @param framework - Optional framework to prefer when auto-detecting
 * @returns Resolved file path with extension, or null if not found
 */
export function resolvePagePath(
  projectPath: string,
  pagePath: string,
  framework?: FrameworkType,
): string | null {
  const ext = path.extname(pagePath);

  // If path already has an extension, check if it exists
  if (ext) {
    const fullPath = path.join(projectPath, pagePath);
    return fs.existsSync(fullPath) ? pagePath : null;
  }

  // No extension - need to detect
  const pageDir = path.dirname(pagePath);
  const baseName = path.basename(pagePath);

  // Determine extension search order
  let extensions: string[];
  if (framework) {
    // Prefer specified framework's extensions, then fallback to others
    const preferredExts = VIEW_EXTENSIONS[framework];
    const otherExts = ALL_VIEW_EXTENSIONS.filter(
      (e) => !preferredExts.includes(e),
    );
    extensions = [...preferredExts, ...otherExts];
  } else {
    // No framework specified - use default order (react first)
    extensions = ALL_VIEW_EXTENSIONS;
  }

  // Try each extension
  for (const extCandidate of extensions) {
    const candidatePath = path.join(pageDir, `${baseName}.${extCandidate}`);
    const fullPath = path.join(projectPath, candidatePath);
    if (fs.existsSync(fullPath)) {
      return candidatePath;
    }
  }

  return null;
}

/**
 * Validate page structure
 */
export function validatePageStructure(
  page: Page,
  pageFiles: PageFiles,
): string[] {
  const errors: string[] = [];

  if (!pageFiles.view.exists) {
    errors.push(`View file not found: ${page.path}`);
  }

  if (!pageFiles.config.exists) {
    errors.push(`Configuration file not found for page: ${page.path}`);
  }

  return errors;
}
