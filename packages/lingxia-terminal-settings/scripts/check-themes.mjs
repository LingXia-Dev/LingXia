import { readFile, readdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const themesSource = await readFile(join(root, "public", "themes.js"), "utf8");
const requiredColors = [
  "background", "foreground", "black", "red", "green", "yellow", "blue",
  "purple", "cyan", "white", "brightBlack", "brightRed", "brightGreen",
  "brightYellow", "brightBlue", "brightPurple", "brightCyan", "brightWhite",
];
const names = (await readdir(join(root, "themes"), { withFileTypes: true }))
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .sort();

if (names.length === 0) throw new Error("at least one packaged theme is required");
for (const name of names) {
  const directory = join(root, "themes", name);
  const meta = JSON.parse(await readFile(join(directory, "meta.json"), "utf8"));
  const scheme = JSON.parse(await readFile(join(directory, "scheme.json"), "utf8"));
  const license = await readFile(join(directory, "LICENSE"), "utf8");
  if (meta.name !== name || !meta.author || !meta.upstream || !meta.spdx) {
    throw new Error(`${name}: meta.json must carry matching name, author, upstream, and spdx`);
  }
  for (const key of requiredColors) {
    if (typeof scheme[key] !== "string" || !/^#[0-9a-f]{6}$/i.test(scheme[key])) {
      throw new Error(`${name}: scheme.json has no valid ${key}`);
    }
  }
  if (!license.trim()) throw new Error(`${name}: LICENSE is empty`);
  if (!themesSource.includes(`name: "${name}"`)) {
    throw new Error(`${name}: public/themes.js is missing the packaged scheme`);
  }
}

console.log(`verified ${names.length} packaged terminal themes`);
