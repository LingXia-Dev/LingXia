import fs from "node:fs";
import path from "node:path";

// Resolve a node_modules/.bin executable, walking up from the start dir so it
// works both for a per-package install (bin in the package's own node_modules)
// and an npm-workspace install (bin hoisted to the workspace root). Falls back
// to the bare name, letting the shell/PATH resolve it.
export function resolveBin(startDir, name) {
  const binName = process.platform === "win32" ? `${name}.cmd` : name;
  let dir = startDir;
  for (;;) {
    const candidate = path.join(dir, "node_modules", ".bin", binName);
    if (fs.existsSync(candidate)) return candidate;
    const parent = path.dirname(dir);
    if (parent === dir) return binName;
    dir = parent;
  }
}
