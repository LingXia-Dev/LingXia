import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";

const generatedFiles = [
  "src/generated/i18n.ts",
  "src/generated/error.ts",
];

const hasGenerated = generatedFiles.every((file) => existsSync(file));
const forceRegenerate = process.env.LINGXIA_REGEN_I18N === "1";

if (hasGenerated && !forceRegenerate) {
  console.log("i18n generated files already exist, skip regeneration");
  process.exit(0);
}

const args = [
  "run",
  "--manifest-path",
  "../Cargo.toml",
  "-p",
  "lingxia-gen",
  "--",
  "i18n",
  "--input",
  "../i18n",
  "--ts-out",
  "src/generated",
];

const result = spawnSync("cargo", args, { stdio: "inherit" });

if (result.error) {
  if (result.error.code === "ENOENT") {
    console.error("cargo is required to generate i18n TypeScript files.");
    console.error(
      "Install Rust toolchain or run build in an environment where cargo is available."
    );
  } else {
    console.error(`failed to run cargo: ${result.error.message}`);
  }
  process.exit(1);
}

process.exit(result.status ?? 1);
