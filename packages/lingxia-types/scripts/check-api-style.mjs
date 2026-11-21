// House style over the Logic API catalogue. Each finding names its rule; the baseline lists the
// findings that predate the rule, and an entry that no longer fires must be removed from it.
// `--write` regenerates the member table and the baseline; the default checks both.
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { catalogue } from "./catalogue.mjs";

const packageDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const baselinePath = resolve(packageDir, "typegen/api-style-baseline.json");
const tablePath = resolve(packageDir, "../../docs/internal/logic-api-members.md");
const write = process.argv.includes("--write");

const rules = {
  "no-get-read": "a read is named for what it returns, without a `get` prefix",
  "subscription-shape": "`watch*` and `on<Event>` return their unsubscribe and take `options?: SignalOptions` last",
  "field-name": "one name per concept: `path` (not `uri`, `filePath`, `tempFilePath`, `outputPath`), `icon` (not `iconPath`), `size` (not `fileSize`, `sizeBytes`, `currentSize`), `…Ms` / `…At` (not `…Seconds`, `…Time`, a bare `duration`, `timeout`, `delay` or `interval`), `type` for MIME (not `mimeType`, `fileType`), `message` (not `body`, `content`), `tone` (not a `style` literal), `appId` (not `appid`), `version` (not `targetVersion`), and none of the mini-program names (`count`, `redDot`, `formData`, `statusCode`, `scene`, `referrerInfo`, `isHighAccuracy`, `coordinateSystem`, `rotate`, `showMenu`, `mask`)",
  "status-values": "a string `status` field is only `ok` or `canceled`; an HTTP `status` number is exempt",
  "readonly-array-param": "an array parameter is `readonly`",
  "required-member": "an API member is never optional; a missing capability rejects `E_NOT_SUPPORTED`",
  "summary": "every member and every option or result field has a one-sentence summary",
  "error-code": "a `@throws` code is a member of `LX_ERROR_CODES` in src/error.ts",
  "tags": "an error code, a `lx.supports` key or a default named in prose is written as `@throws {E_…}`, `@supports key` or `@default value`",
  "doc-length": "a member's doc is at most 12 lines; details go on its option and result fields",
  "jargon": "a summary says what the member does, without its own path or internal terms",
  "argument-shape": "a member takes `(subject?, options?)`: at most one positional subject (a callback for `watch`/`on<Event>`), then one `options` or `patch` object last; no boolean flag argument",
};

const { members, types } = catalogue();
const findings = [];
const find = (rule, path) => findings.push(`${rule} ${path}`);
const proseDefault = /(^|[.;:(]\s*)Default\b|\(default\b|\bdefaults? (to|is)\b|\bby default\b/;
const jargon = /^`?lx\.[\w.]+\(|\b(Rong|js_api|LogicRoute|native-assigned|appservice|bizCode|generation-scoped|event bus)\b/;
function checkDoc(path, item) {
  if (/\bE_[A-Z_]+/.test(item.doc) && !item.tags.includes("throws")) find("tags", path);
  if (/lx\.supports\(/.test(item.doc) && !item.tags.includes("supports")) find("tags", path);
  if (proseDefault.test(item.doc) && !item.tags.includes("default")) find("tags", path);
  if (jargon.test(item.summary)) find("jargon", path);
}
// `(subject?, options?)`: the trailing `options`/`patch` object is the options slot, every other
// parameter a subject. An overload whose only parameter is `never` exists to reject a call.
const optionsSlot = parameter => parameter.name === "options" || parameter.name === "patch";
function shapeOf(signature) {
  const parameters = signature.parameters;
  if (parameters.length === 1 && parameters[0].type === "never") return null;
  const last = parameters.at(-1);
  const options = last && optionsSlot(last) ? last : undefined;
  const subjects = options ? parameters.slice(0, -1) : parameters;
  const fits = subjects.length <= 1 && !parameters.some(p => p.type === "boolean")
    && !subjects.some(optionsSlot);
  const slot = p => `${p.name}${p.optional ? "?" : ""}`;
  return { fits, text: `(${[...subjects, ...(options ? [options] : [])].map(slot).join(", ")})` };
}
const shapeText = member => [...new Set(member.signatures.map(shapeOf).filter(Boolean).map(shape => shape.text))].join("<br>");

const staleFieldName = new RegExp("^(" + [
  "uri", "filePath", "tempFilePath", "outputPath", "iconPath", "fileSize", "sizeBytes", "currentSize", "limitSize",
  "duration", "timeout", "delay", "interval", "mimeType", "fileType", "body", "content", "appid", "targetVersion",
  "count", "redDot", "formData", "statusCode", "scene", "referrerInfo", "isHighAccuracy", "coordinateSystem",
  "rotate", "showMenu", "mask",
].join("|") + ")$|Seconds$|[a-z]Time$");

// Brands and exclusivity guards (`page?: never`) are not fields a person reads.
const isGuard = field => field.name.startsWith("__@") || field.type === "undefined" || field.type === "never";

for (const member of members) {
  const name = member.path.split(".").pop();
  if (!member.summary) find("summary", member.path);
  checkDoc(member.path, member);
  if (member.doc.split("\n").length + member.tags.length > 12) find("doc-length", member.path);
  if (member.optional) find("required-member", member.path);
  if (member.kind !== "method") continue;
  if (/^get[A-Z]/.test(name)) find("no-get-read", member.path);
  if (member.signatures.some(signature => shapeOf(signature)?.fits === false)) find("argument-shape", member.path);
  if (/^(watch\w*|on[A-Z]\w*)$/.test(name)) {
    const shaped = member.signatures.every(signature =>
      signature.returns === "() => void" && /SignalOptions/.test(signature.parameters.at(-1)?.type ?? ""));
    if (!shaped) find("subscription-shape", member.path);
  }
  for (const signature of member.signatures) {
    for (const parameter of signature.parameters) {
      if (/(^|[^\w.])(?!readonly )\w+\[\]|(^|[^y])Array</.test(parameter.type) && !/^readonly /.test(parameter.type))
        find("readonly-array-param", `${member.path}(${parameter.name})`);
    }
  }
}
for (const [type, fields] of types) {
  for (const field of fields) {
    if (isGuard(field)) continue;
    const path = `${type}.${field.name}`;
    if (!field.summary) find("summary", path);
    checkDoc(path, field);
    if (staleFieldName.test(field.name) || (field.name === "style" && /^"/.test(field.type))) find("field-name", path);
    if (field.name === "status" && field.type !== "number") {
      const values = [...field.type.matchAll(/"([^"]+)"/g)].map(([, value]) => value);
      if (values.some(value => value !== "ok" && value !== "canceled")) find("status-values", path);
    }
  }
}

const table = [
  "# Logic API members",
  "",
  "Generated by `npm run gen:api` in `packages/lingxia-types` from the published declarations. Do not edit.",
  "",
  "| Member | Shape | Signature | Summary |",
  "| --- | --- | --- | --- |",
  ...members.map(member => `| \`${member.path}\` | ${member.kind === "method" ? `\`${shapeText(member)}\`` : ""} | ${member.signatures.map(signature => `\`${signature.text.replace(/\|/g, "\\|")}\``).join("<br>") || member.kind} | ${member.summary.replace(/\|/g, "\\|")} |`),
  "",
].join("\n");

// A `@throws` code must be one `LxErrorCode` names, or `e.code === …` on it would not type-check.
const declaredCodes = new Set(readFileSync(resolve(packageDir, "src/error.ts"), "utf8").match(/LX_ERROR_CODES = \[([^]*?)\]/)[1].match(/E_[A-Z_]+/g));
for (const file of ["src/generated/logic.ts", "src/page.ts", "src/index.ts"]) {
  for (const [, code] of readFileSync(resolve(packageDir, file), "utf8").matchAll(/@throws \{(E_[A-Z_]+)\}/g)) {
    if (!declaredCodes.has(code)) find("error-code", `${file}:${code}`);
  }
}

const current = [...new Set(findings)].sort();
if (write) {
  const byRule = Object.fromEntries(Object.keys(rules).map(rule => [rule, current.filter(f => f.startsWith(`${rule} `)).map(f => f.slice(rule.length + 1))]));
  writeFileSync(baselinePath, `${JSON.stringify(byRule, null, 2)}\n`);
  writeFileSync(tablePath, table);
  console.log(`wrote ${tablePath} and the baseline (${current.length} findings)`);
  process.exit(0);
}

const baseline = JSON.parse(readFileSync(baselinePath, "utf8"));
const allowed = new Set(Object.entries(baseline).flatMap(([rule, paths]) => paths.map(path => `${rule} ${path}`)));
const fresh = current.filter(finding => !allowed.has(finding));
const stale = [...allowed].filter(finding => !current.includes(finding));
let failed = false;
if (fresh.length) {
  failed = true;
  console.error(`API style: ${fresh.length} new finding(s):`);
  for (const finding of fresh) {
    const [rule, path] = finding.split(" ");
    console.error(`  ${path}: ${rule}: ${rules[rule]}`);
  }
}
if (stale.length) {
  failed = true;
  console.error(`API style: fixed, so remove from typegen/api-style-baseline.json:\n  ${stale.join("\n  ")}`);
}
if (readFileSync(tablePath, "utf8") !== table) {
  failed = true;
  console.error("docs/internal/logic-api-members.md is stale; run `npm run gen:api`");
}
if (failed) process.exit(1);
const covered = members.length + [...types.values()].flat().filter(field => !isGuard(field)).length;
console.log(`API style: ${members.length} members, ${types.size} option and result types; ${current.length} baseline findings (${current.filter(f => f.startsWith("summary ")).length} without a summary of ${covered}).`);
