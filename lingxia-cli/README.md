# LingXia CLI

The LingXia CLI provides the command-line entry point for building LingXia applications alongside the workspace builder tooling. The package is currently consumed locally and is not yet published to npm.

## Usage

```bash
lingxia build [--dev | --prod]
```

The CLI delegates the build workflow to the internal `lingxia-builder` package.

## Local Development

Install dependencies and produce the compiled command-line bundle:

```bash
npm install --prefix ../lingxia-builder
npm install --prefix .
npm run build
```

After the build completes you can invoke the CLI directly:

```bash
./bin/lingxia.js --help
```

Update `src/index.ts` when you need to adjust CLI behavior, then re-run `npm run build` to refresh the distribution artifacts.
