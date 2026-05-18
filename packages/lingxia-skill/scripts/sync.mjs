#!/usr/bin/env node
// Sync the canonical skill source from docs/skill/ into packages/lingxia-skill/skill/.
//
// Run during `npm run sync` (manual) and `prepack` (automatic before publish).
// The published tarball contains only the synced `skill/` copy — the canonical
// `docs/skill/` source is not part of the published package.
//
// If the canonical source is not reachable (e.g. consumers running scripts
// from the installed package), the sync is skipped silently — the `skill/`
// directory already shipped in the tarball is authoritative.

import { cp, mkdir, rm, stat, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const pkgRoot = resolve(here, "..");
const repoRoot = resolve(pkgRoot, "..", "..");
const source = join(repoRoot, "docs", "skill");
const target = join(pkgRoot, "skill");

async function exists(p) {
  try {
    await stat(p);
    return true;
  } catch {
    return false;
  }
}

async function main() {
  if (!(await exists(source))) {
    // Running outside the monorepo (e.g. inside an installed @lingxia/skill
    // tarball). Nothing to sync; ship as-is.
    console.log(
      `[sync] skipped — source not found at ${source} (expected when running from an installed package)`
    );
    return;
  }

  if (await exists(target)) {
    await rm(target, { recursive: true, force: true });
  }
  await mkdir(target, { recursive: true });
  await cp(source, target, { recursive: true });

  // Write a small manifest so consumers can introspect the skill version
  // without reading SKILL.md.
  const pkgJson = JSON.parse(
    await readFile(join(pkgRoot, "package.json"), "utf8")
  );
  const manifest = {
    name: pkgJson.name,
    version: pkgJson.version,
    entry: "SKILL.md",
    syncedAt: new Date().toISOString(),
  };
  await writeFile(
    join(target, "skill-manifest.json"),
    JSON.stringify(manifest, null, 2) + "\n"
  );

  console.log(`[sync] copied ${source} → ${target}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
