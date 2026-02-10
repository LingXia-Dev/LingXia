import { Command } from "commander";
import { createRequire } from "module";
import { buildCommand } from "./builder/index.js";
import { fileURLToPath, pathToFileURL } from "url";
import path from "path";

const { version } = createRequire(import.meta.url)("../package.json");

export async function runCLI(argv = process.argv): Promise<void> {
  const program = new Command();
  program
    .name("lingxia")
    .description("LingXia LxApp build tools")
    .version(version ?? "0.0.0");

  program
    .command("build")
    .description("Build LingXia project or plugin")
    .option("--release", "Release build (debug is the default)")
    .option(
      "--package",
      "Package dist output into an archive (requires --release)",
    )
    .option(
      "--target <target>",
      "JS target (es5, es2015, es2020, esnext). Note: es5 requires @vitejs/plugin-legacy",
    )
    .option(
      "--framework <framework>",
      "Framework to use for pages without extension (react or vue). Auto-detected if not specified.",
    )
    .action(buildCommand);

  await program.parseAsync(argv);
}

const isMain = (() => {
  if (!process.argv[1]) return false;
  const mainPath = path.resolve(process.argv[1]);
  const modulePath = fileURLToPath(import.meta.url);
  return (
    mainPath === modulePath || pathToFileURL(mainPath).href === import.meta.url
  );
})();

if (isMain) {
  runCLI().catch((err) => {
    console.error(err instanceof Error ? err.message : String(err));
    process.exit(1);
  });
}
