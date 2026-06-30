#!/usr/bin/env node
// `npx @lingxia/skill install [target]`
//
// Copies the LingXia agent skill into the chosen target. The skill is plain
// markdown — readable by Claude Code, Claude Agent SDK, OpenAI Codex, Cursor,
// or any tool that consumes project markdown — and ships in the layout that
// Anthropic Skills expect (`<target>/lingxia/SKILL.md` + relative sub-files).
//
// Default targets:
//   - <cwd>/.claude/skills/lingxia/     (project-scoped, default)
//   - ~/.claude/skills/lingxia/         (--user)
//   - <path>/lingxia/                   (--target <path>)
//
// For tools that look for a single AGENTS.md at the repo root (e.g. Codex CLI),
// pass --agents-md to also write a tiny pointer file at <cwd>/AGENTS.md that
// directs the agent at the installed SKILL.md.
//
// The skill source ships inside this package at `skill/`. It is synced from
// `docs/skill/` in the LingXia monorepo at publish time. Reinstalling is
// idempotent: existing target contents are removed first (unless --dry-run).

import { cp, mkdir, readFile, rm, stat, readdir, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { homedir } from "node:os";

const here = dirname(fileURLToPath(import.meta.url));
const pkgRoot = resolve(here, "..");
const source = join(pkgRoot, "skill");

function parseArgs(argv) {
  const args = {
    _: [],
    user: false,
    force: false,
    dryRun: false,
    target: null,
    agentsMd: false,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === "--user") args.user = true;
    else if (a === "--force") args.force = true;
    else if (a === "--dry-run" || a === "-n") args.dryRun = true;
    else if (a === "--target") args.target = argv[++i];
    else if (a === "--agents-md") args.agentsMd = true;
    else if (a === "--help" || a === "-h") args.help = true;
    else if (a === "--version" || a === "-V") args.version = true;
    else args._.push(a);
  }
  return args;
}

function printHelp() {
  console.log(`Usage: npx @lingxia/skill <command> [options]

Install the LingXia agent skill into a project. The content is plain markdown
and works with any AI coding tool that reads project files (Claude Code,
Claude Agent SDK, OpenAI Codex, Cursor, ...).

Commands:
  install            Copy the skill into a target directory (default)
  uninstall          Remove an installed copy
  where              Print the path where install would write
  version            Print the skill version

Options:
  --user             Install into ~/.claude/skills/lingxia/ instead of <cwd>/.claude/skills/lingxia/
  --target <path>    Install into <path>/lingxia/ (overrides --user and cwd)
  --agents-md        Also write <cwd>/AGENTS.md pointing at the installed skill
                     (handy for Codex CLI and other tools that read AGENTS.md)
  --force            Overwrite an existing install without prompting (default in non-interactive shells)
  --dry-run, -n      Print actions without writing
  -h, --help         Show help
  -V, --version      Show version

Examples:
  npx @lingxia/skill install                                # Anthropic-style: .claude/skills/lingxia/
  npx @lingxia/skill install --user                         # ~/.claude/skills/lingxia/
  npx @lingxia/skill install --agents-md                    # also write AGENTS.md (Codex-friendly)
  npx @lingxia/skill install --target docs/agent --dry-run  # custom target, preview only
  npx @lingxia/skill uninstall
`);
}

async function readVersion() {
  const pkg = JSON.parse(
    await readFile(join(pkgRoot, "package.json"), "utf8")
  );
  return pkg.version;
}

async function exists(p) {
  try {
    await stat(p);
    return true;
  } catch {
    return false;
  }
}

function resolveTarget(args) {
  if (args.target) {
    const base = isAbsolute(args.target) ? args.target : resolve(process.cwd(), args.target);
    return join(base, "lingxia");
  }
  if (args.user) {
    return join(homedir(), ".claude", "skills", "lingxia");
  }
  return join(process.cwd(), ".claude", "skills", "lingxia");
}

async function ensureSourceExists() {
  if (!(await exists(source))) {
    console.error(
      `error: skill source missing at ${source}\n` +
        "       this package may not have been built/synced before publish."
    );
    process.exit(1);
  }
}

async function install(args) {
  await ensureSourceExists();
  const target = resolveTarget(args);
  const targetExists = await exists(target);
  const agentsMdPath = args.agentsMd ? join(process.cwd(), "AGENTS.md") : null;

  console.log(`source: ${source}`);
  console.log(`target: ${target}`);
  if (agentsMdPath) console.log(`agents-md: ${agentsMdPath}`);

  if (args.dryRun) {
    console.log("[dry-run] would " + (targetExists ? "replace" : "create") + " target");
    if (agentsMdPath) {
      console.log(
        "[dry-run] would " +
          ((await exists(agentsMdPath)) ? "append to" : "write") +
          " AGENTS.md"
      );
    }
    return;
  }

  if (targetExists) {
    await rm(target, { recursive: true, force: true });
  }
  await mkdir(target, { recursive: true });
  await cp(source, target, { recursive: true });

  if (agentsMdPath) {
    await writeAgentsMd(agentsMdPath, target);
  }

  console.log(`installed @lingxia/skill v${await readVersion()} → ${target}`);
  console.log(
    "The skill is plain markdown; open SKILL.md or point your AI coding tool at the install directory."
  );
}

async function writeAgentsMd(agentsMdPath, skillTarget) {
  // Reference the skill portably so a committed AGENTS.md has no machine-specific
  // absolute path: project-relative when the skill lives inside the project, or
  // home-relative (`~`) for a global `--user` install (every user's `~` resolves
  // locally), falling back to the absolute path only as a last resort.
  const home = homedir();
  let skillRef;
  if (skillTarget.startsWith(process.cwd() + "/")) {
    skillRef = skillTarget.slice(process.cwd().length + 1);
  } else if (skillTarget === home || skillTarget.startsWith(home + "/")) {
    skillRef = "~" + skillTarget.slice(home.length);
  } else {
    skillRef = skillTarget;
  }
  const marker = "<!-- @lingxia/skill: AGENTS.md pointer -->";
  const block = `${marker}
## LingXia

This project uses the LingXia cross-platform app framework. The development
skill — decision tree, recipes, CLI / component / native API references —
lives at:

    ${skillRef}/SKILL.md

If that file is not present, install it once with:

    npx @lingxia/skill install --user

Start there. Sub-references are linked from that file using relative paths.
${marker}
`;

  if (await exists(agentsMdPath)) {
    const existing = await readFile(agentsMdPath, "utf8");
    if (existing.includes(marker)) {
      // Replace the previous block in-place.
      const re = new RegExp(
        `${marker}[\\s\\S]*?${marker}\\n?`,
        "g"
      );
      await writeFile(agentsMdPath, existing.replace(re, block));
    } else {
      const sep = existing.endsWith("\n") ? "\n" : "\n\n";
      await writeFile(agentsMdPath, existing + sep + block);
    }
  } else {
    await writeFile(agentsMdPath, `# AGENTS\n\n${block}`);
  }
}

async function uninstall(args) {
  const target = resolveTarget(args);
  if (!(await exists(target))) {
    console.log(`nothing to uninstall at ${target}`);
    return;
  }
  if (args.dryRun) {
    console.log(`[dry-run] would remove ${target}`);
    return;
  }
  await rm(target, { recursive: true, force: true });
  console.log(`removed ${target}`);
}

async function where(args) {
  console.log(resolveTarget(args));
}

async function main() {
  const argv = process.argv.slice(2);
  const args = parseArgs(argv);

  if (args.help) return printHelp();
  if (args.version) {
    console.log(await readVersion());
    return;
  }

  const cmd = args._[0] ?? "install";

  switch (cmd) {
    case "install":
      return install(args);
    case "uninstall":
      return uninstall(args);
    case "where":
      return where(args);
    case "version":
      console.log(await readVersion());
      return;
    default:
      console.error(`unknown command: ${cmd}\n`);
      printHelp();
      process.exit(2);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
