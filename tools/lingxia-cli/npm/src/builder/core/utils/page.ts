import type { Page, PageFiles } from "../../types/index.js";
import { FileUtils } from "./file.js";

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
