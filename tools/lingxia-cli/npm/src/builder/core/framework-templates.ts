import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";

export interface FrameworkTemplates {
  indexHtml: string;
  mainEntry: string;
  mainEntryFilename: string;
}

interface FrameworkTemplateMeta {
  dir: string;
  indexHtmlFile: string;
  mainEntryFile: string;
}

const FRAMEWORK_TEMPLATE_META: Record<string, FrameworkTemplateMeta> = {
  react: {
    dir: "react",
    indexHtmlFile: "index.html",
    mainEntryFile: "main.jsx",
  },
  vue: {
    dir: "vue",
    indexHtmlFile: "index.html",
    mainEntryFile: "main.js",
  },
};

const moduleDir = path.dirname(fileURLToPath(import.meta.url));
const templateRootCandidates = [
  // Built npm package: dist/builder/core -> templates/builder-frameworks
  path.resolve(moduleDir, "../../../templates/builder-frameworks"),
  // Source execution in local dev/tests: src/builder/core -> templates/builder-frameworks
  path.resolve(moduleDir, "../../../../templates/builder-frameworks"),
];

let templateRootDir: string | null | undefined;
const templateCache = new Map<string, FrameworkTemplates>();

function resolveTemplateRootDir(): string | null {
  if (templateRootDir !== undefined) {
    return templateRootDir;
  }

  for (const candidate of templateRootCandidates) {
    if (fs.existsSync(candidate)) {
      templateRootDir = candidate;
      return templateRootDir;
    }
  }

  templateRootDir = null;
  return null;
}

function readTemplate(framework: string): FrameworkTemplates | undefined {
  const meta = FRAMEWORK_TEMPLATE_META[framework];
  if (!meta) return undefined;

  const rootDir = resolveTemplateRootDir();
  if (!rootDir) {
    throw new Error(
      `Framework template root not found. Checked: ${templateRootCandidates.join(", ")}`,
    );
  }

  const frameworkDir = path.join(rootDir, meta.dir);
  const indexHtmlPath = path.join(frameworkDir, meta.indexHtmlFile);
  const mainEntryPath = path.join(frameworkDir, meta.mainEntryFile);

  if (!fs.existsSync(indexHtmlPath) || !fs.existsSync(mainEntryPath)) {
    return undefined;
  }

  return {
    indexHtml: fs.readFileSync(indexHtmlPath, "utf-8"),
    mainEntry: fs.readFileSync(mainEntryPath, "utf-8"),
    mainEntryFilename: meta.mainEntryFile,
  };
}

export function getFrameworkTemplates(
  framework: string,
): FrameworkTemplates | undefined {
  if (templateCache.has(framework)) {
    return templateCache.get(framework);
  }

  const templates = readTemplate(framework);
  if (templates) {
    templateCache.set(framework, templates);
  }
  return templates;
}

export function hasFrameworkTemplates(framework: string): boolean {
  const meta = FRAMEWORK_TEMPLATE_META[framework];
  if (!meta) return false;

  const rootDir = resolveTemplateRootDir();
  if (!rootDir) return false;

  const frameworkDir = path.join(rootDir, meta.dir);
  return (
    fs.existsSync(path.join(frameworkDir, meta.indexHtmlFile)) &&
    fs.existsSync(path.join(frameworkDir, meta.mainEntryFile))
  );
}
