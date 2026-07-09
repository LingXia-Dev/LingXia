# @lingxia/skill

The development skill for the [LingXia](https://github.com/LingXia-Dev/LingXia) cross-platform app framework, as a plain-markdown bundle you can install into any project.

Open it with any AI coding tool that reads project files — Claude Code, the Claude Agent SDK, OpenAI Codex CLI, Cursor — and it will route the agent through LingXia's decision tree, recipes, CLI / component / native-API references as you build:

- **standalone lxapps** — page-based mini-apps with a View + Logic split (React, Vue, or HTML view; JS Logic)
- **native host apps** — Android, iOS, macOS, and Harmony shells embedding an lxapp
- **Rust native extensions** — `#[lingxia::native]` routes and `lingxia::js` AppService extensions

The on-disk layout follows the [Anthropic Skills convention](https://docs.claude.com/en/docs/claude-code/skills) — a `SKILL.md` entry with a frontmatter manifest plus relative sub-files. Tools that don't use that convention can still read the content directly; see _Codex CLI and other tools_ below.

## Install

The skill is opt-in — `lingxia new` doesn't bundle it, only prints a hint pointing at the install command below. Install it once per project (or globally per user):

```bash
# Project-scoped: ./.claude/skills/lingxia/
npx @lingxia/skill install

# User-scoped: ~/.claude/skills/lingxia/
npx @lingxia/skill install --user

# Custom location: <path>/lingxia/
npx @lingxia/skill install --target ./agent

# Preview without writing
npx @lingxia/skill install --dry-run
```

Then open the project in your AI tool of choice. Claude tools discover `.claude/skills/` directly. Codex and many other agents should be given an `AGENTS.md` pointer; see the next section.

### Codex CLI and other tools

OpenAI Codex CLI reads repository instructions from `AGENTS.md`; it does not rely on Claude's `.claude/skills/` discovery path. Pass `--agents-md` and the installer writes the skill normally, then adds a small pointer block to `<project>/AGENTS.md` directing Codex at the installed `SKILL.md`:

```bash
npx @lingxia/skill install --agents-md
```

The block is fenced with HTML markers, so re-running the installer replaces it in place rather than appending duplicates. If the file doesn't exist yet, it's created; if it already has content, the pointer is appended.

For Cursor, GitHub Copilot, or any agent that reads project markdown on demand, the default install plus a short rule in `.cursorrules` / `.github/copilot-instructions.md` pointing at `.claude/skills/lingxia/SKILL.md` works fine.

### Other commands

```bash
npx @lingxia/skill where        # print the target install path
npx @lingxia/skill uninstall    # remove the installed skill directory
npx @lingxia/skill version      # print the skill version
npx @lingxia/skill --help
```

(`uninstall` leaves `AGENTS.md` untouched — humans may have added content to it. Remove the LingXia block manually if you want it gone.)

## Versioning

`@lingxia/skill` ships with the same version number as the `lingxia` CLI and the rest of `@lingxia/*`. Pin a specific version when reproducibility matters:

```bash
npx @lingxia/skill@0.7.0 install
```

The skill's content documents the CLI surface; CLI releases that change a flag bump the skill version in lockstep.

## What's inside

```
.claude/skills/lingxia/
├── SKILL.md              # entry: decision tree, fast-path recipes, symptom router
├── cli/
│   ├── lingxia.md        # the lingxia CLI: build, dev sessions, package, install
│   ├── lxdev.md          # drive a running dev session (reload, eval, automation, logs)
│   ├── distribution.md   # publish, app-store submission, developer accounts
│   └── signing.md        # platform signing setup
├── lxapp/
│   ├── guide.md          # Page({}), useLxPage, events
│   ├── components.md     # LxInput, LxVideo, LxPicker, LxMediaSwiper, LxNavigator
│   ├── lx-api.md         # lx.* surface map + how to install @lingxia/types
│   └── bridge.md         # setData, stream, channel mechanics
├── app/
│   ├── project.md        # lingxia.yaml + macOS App UI
│   ├── apple-sdk.md      # iOS/macOS SDK embedding
│   └── applinks.md       # universal links / app links
├── native/development.md # Rust host: #[lingxia::native], HostAddon, facades
├── reference/
│   └── file-lifecycle.md # storage classes, downloadFile, FileManager
└── skill-manifest.json   # package name + version + sync timestamp
```

## Where the content lives

The canonical source is in the LingXia monorepo at [`docs/skill/`](https://github.com/LingXia-Dev/LingXia/tree/main/docs/skill). At publish time, `scripts/sync.mjs` copies that tree into `skill/` inside this package. End users of `@lingxia/skill` consume the synced copy.

The skill itself doesn't depend on the LingXia repo at runtime — once installed, all cross-references are relative paths inside `skill/`.

## License

MIT
