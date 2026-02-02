import fs from "fs";
import path from "path";

export type ProjectFramework = "react" | "vue" | "html";

export function readProjectFramework(projectPath: string): ProjectFramework {
  const pkgPath = path.join(projectPath, "package.json");
  if (fs.existsSync(pkgPath)) {
    try {
      const packageJson = JSON.parse(fs.readFileSync(pkgPath, "utf-8"));
      const framework = packageJson?.lingxia?.framework;
      if (framework === "react" || framework === "vue" || framework === "html") {
        return framework;
      }
    } catch {
      // ignore
    }
  }

  const detected = detectFrameworkFromPages(projectPath);
  if (detected) return detected;

  throw new Error(
    `Cannot determine Lingxia project framework. Please rerun ` +
      `'lingxia create' or add {"lingxia":{"framework":"react|vue|html"}} to package.json.`,
  );
}

function detectFrameworkFromPages(
  projectPath: string,
): ProjectFramework | undefined {
  const pagesDir = path.join(projectPath, "pages");
  if (!fs.existsSync(pagesDir)) {
    return undefined;
  }
  const entries = fs.readdirSync(pagesDir, { withFileTypes: true });
  for (const entry of entries) {
    if (!entry.isDirectory()) continue;
    const dir = path.join(pagesDir, entry.name);
    const reactEntry = findFirstFileWithExt(dir, [".tsx", ".jsx"]);
    if (reactEntry) return "react";
    const vueEntry = findFirstFileWithExt(dir, [".vue"]);
    if (vueEntry) return "vue";
    const htmlEntry = findFirstFileWithExt(dir, [".html"]);
    if (htmlEntry) return "html";
  }
  return undefined;
}

function findFirstFileWithExt(
  dir: string,
  extensions: string[],
): string | undefined {
  for (const ext of extensions) {
    const candidate = path.join(dir, `index${ext}`);
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }
  return undefined;
}
