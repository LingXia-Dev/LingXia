# LingXia CLI

The LingXia CLI scaffolds, builds, and packages LingXia mini-apps. It bundles view/logic layers with Vite, cleans temporary artifacts, and exports a final `<lxAppId>.zip` ready for distribution.

## Requirements

- Node.js ≥ 18
- npm (or pnpm/yarn) for project dependencies

## Quick Start

```bash
# scaffold a new project
npx @lingxia/cli create my-app --framework react

cd my-app
npm install

# build view + logic and get a ready-to-ship zip
npx @lingxia/cli build --prod
```

The build command:

- Executes both logic and view builders (use `LINGXIA_ONLY=logic|view` to limit scope).
- Copies `lxapp.json` and resolves `lxapp.css` imports into `dist/`.
- Removes the temporary `.lingxia-build` directory after success.
- Produces `<projectRoot>/dist/` plus a packaged `<lxAppId>.zip` beside the project.

## Commands

### `lingxia create [name] [--framework react|vue]`

Scaffolds a new LingXia project from the templates in `templates/create/`. If no name/framework is provided, you’ll be prompted interactively. The script:

1. Copies the selected template.
2. Replaces placeholders (package name, app id, display name).
3. Records the chosen framework under `package.json.lingxia.framework`.

### `lingxia build [--dev | --prod]`

Builds the active project in the current working directory.

Options & environment flags:

- `--dev` / `--prod`: forwards mode to the builders (defaults to prod).
- `LINGXIA_ONLY=logic` or `LINGXIA_ONLY=view`: build a single layer.

Outputs:

- `dist/` containing page bundles, `lxapp.json`, `lxapp.css`, static assets, and page configs.
- `<lxAppId>.zip` (derived from `lxapp.json.lxAppId`) with the contents of `dist/`.

## Project Structure Expectations

- `lxapp.json`: page list and metadata (required).
- `pages/**`: React/Vue entries per page.
- `lxapp.css`: optional global styles; `@import` statements are resolved during build.
- `.lingxia-build/`: internal Vite output cleaned automatically after each build.

## Local CLI Development

```bash
cd lingxia-cli
npm install
npm run build   # emits dist/ + declarations
npm test        # optional, runs vitest
./bin/lingxia.js --help
```

Update `src/**` (e.g., `src/commands/create.ts`, `src/builder/**`) and re-run `npm run build` whenever you change CLI behavior.

## Contributing

- Run `npm run build` before submitting changes so `dist/` stays in sync.
- Keep templates under `templates/` framework-agnostic and free of hard-coded local paths.
- Prefer Cheerio/DOM-based HTML rewrites over regex replacements to avoid brittle path issues.

For issues or feature requests, open a ticket in the LingXia repository. Happy building!
