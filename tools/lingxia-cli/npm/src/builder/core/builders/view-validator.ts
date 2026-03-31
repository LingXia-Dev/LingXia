import fs from "fs";
import path from "path";
import type { PageFiles } from "../../types/index.js";

/** Pattern to match lx.xxx API calls */
const LX_API_PATTERN = /\blx\.[a-zA-Z_]\w*/g;

/**
 * Validate that view files don't use `lx.*` APIs directly.
 * These APIs should only be used in the logic layer (index.ts).
 *
 * @throws Error if `lx.*` APIs are found in view files
 */
export function validateViewFile(pageFiles: PageFiles): void {
  if (!pageFiles.view.exists || !pageFiles.view.path) return;

  const viewPath = pageFiles.view.path;
  const content = fs.readFileSync(viewPath, "utf-8");
  const cleaned = stripCommentsAndStrings(content);
  const matches = findMatches(cleaned);

  if (matches.length === 0) return;

  const fileName =
    path.basename(path.dirname(viewPath)) + "/" + path.basename(viewPath);
  const lines = matches.map((m) => `  Line ${m.line}: ${m.text}`).join("\n");

  throw new Error(
    `\n❌ \`lx.*\` API cannot be used in view files: ${fileName}\n\n` +
      `${lines}\n\n` +
      `Move these calls to the logic layer, then access data via useLxPage().\n`,
  );
}

function findMatches(content: string): { line: number; text: string }[] {
  const results: { line: number; text: string }[] = [];
  const lines = content.split("\n");

  for (let i = 0; i < lines.length; i++) {
    LX_API_PATTERN.lastIndex = 0;
    let match;
    while ((match = LX_API_PATTERN.exec(lines[i])) !== null) {
      results.push({ line: i + 1, text: match[0] });
    }
  }
  return results;
}

/** Remove comments, strings, and JSX text content to avoid false positives */
function stripCommentsAndStrings(content: string): string {
  return (
    content
      .replace(/\/\/.*$/gm, (m) => " ".repeat(m.length)) // single-line comments
      .replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " ")) // multi-line comments
      .replace(/`(?:[^`\\]|\\.)*`/gs, (m) => m.replace(/[^\n]/g, " ")) // template literals
      .replace(/"(?:[^"\\]|\\.)*"/g, (m) => " ".repeat(m.length)) // double-quoted strings
      .replace(/'(?:[^'\\]|\\.)*'/g, (m) => " ".repeat(m.length)) // single-quoted strings
      .replace(/<!--[\s\S]*?-->/g, (m) => m.replace(/[^\n]/g, " ")) // HTML comments
      // JSX/HTML text content between tags: strip pure display text but keep code
      // Match content between > and <, but not inside script/style tags
      .replace(/>([^<]*)<(?!\/(?:script|style))/gi, (match, text) => {
        // Keep if it contains code patterns (assignments, statements, blocks)
        if (/[=;{}]/.test(text)) {
          return match;
        }
        // Strip pure display text (like button labels)
        return ">" + " ".repeat(text.length) + "<";
      })
  );
}
