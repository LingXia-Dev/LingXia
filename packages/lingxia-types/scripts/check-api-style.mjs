// House style over the Logic API catalogue. Each finding names its rule; the baseline lists the
// findings that predate the rule, and an entry that no longer fires must be removed from it.
// `--write` regenerates the member table and the baseline; the default checks both.
// `--only rule,…` checks just those rules against the baseline, without the table.
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { catalogue } from "./catalogue.mjs";

const packageDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const baselinePath = resolve(packageDir, "typegen/api-style-baseline.json");
const tablePath = resolve(packageDir, "../../docs/internal/logic-api-members.md");
const write = process.argv.includes("--write");
const onlyArg = process.argv.find(arg => arg.startsWith("--only="));
const only = onlyArg ? new Set(onlyArg.slice("--only=".length).split(",")) : null;

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
  "literal-case": "a multi-word string literal is snake_case and a one-word literal lower case; HTTP methods and `LxErrorReason` are exempt",
  "readonly-array-field": "an array field of an option or result type is `readonly`",
  "url-type": "a `url` parameter or field accepts `string | URL`",
  "supports-key": "a member that can reject `E_NOT_SUPPORTED` names its `@supports` key, and every key is one `lx.supports` answers (crates/lingxia-logic/src/features.json)",
  "throws": "a member returning a Promise or a task names what it rejects with as `@throws {E_…}`, or says it never rejects",
  "type-summary": "every named type has a one-sentence summary",
  "namespace-example": "every top-level `lx` namespace has an `@example` on one of its calls",
  "tag-vocabulary": "tags are `@throws {E_…} when`, `@default value`, `@platform <iOS|Android|HarmonyOS|macOS|Windows>[, …]: text` (one tag per platform group), `@permission name`, `@supports key`, `@example` with a fenced block, `@param`, `@see` or `@deprecated`",
};

const { members, types } = catalogue();
// The whole authored surface (page API, `App`, error helpers), for summaries only.
const full = catalogue({ entries: true });
const featureKeys = new Set(JSON.parse(readFileSync(resolve(packageDir, "../../crates/lingxia-logic/src/features.json"), "utf8")).map(entry => entry.key));
const findings = [];
const find = (rule, path) => findings.push(`${rule} ${path}`);
const proseDefault = /(^|[.;:(]\s*)Default\b|\(default\b|\bdefaults? (to|is)\b|\bby default\b/;
const jargon = /^`?lx\.[\w.]+\(|\b(Rong|js_api|LogicRoute|native-assigned|appservice|bizCode|generation-scoped|event bus)\b/;
// Drop every `readonly` array or tuple first, so a nested mutable one still shows.
function withoutReadonly(type) {
  let out = "";
  for (let i = 0; i < type.length;) {
    if (!type.startsWith("readonly ", i)) { out += type[i++]; continue; }
    let j = i + 9;
    const open = type[j];
    if ("({[".includes(open)) {
      const close = { "(": ")", "{": "}", "[": "]" }[open];
      for (let depth = 0; j < type.length; j++) {
        if (type[j] === open) depth++;
        else if (type[j] === close && --depth === 0) { j++; break; }
      }
    } else {
      while (j < type.length && /[\w.<>]/.test(type[j])) j++;
    }
    while (type.startsWith("[]", j)) j += 2;
    i = j;
  }
  return out;
}
const isMutableArray = type => /\[\]|(?<!Readonly)Array<|^\[|[|(] *\[/.test(withoutReadonly(type));
const isBareUrl = type => type === "string" || type === "string | undefined";
const PLATFORM = "(?:iOS|Android|HarmonyOS|macOS|Windows)";
const platformTag = new RegExp(`^${PLATFORM}(?:, ${PLATFORM})*(?: [^:]+)?: \\S`);
const secondPlatform = new RegExp(`[.;] ${PLATFORM}(?:, ${PLATFORM})*: `);
const tagShape = {
  throws: text => /^\{E_[A-Z_]+\}\s+\S/.test(text),
  default: text => text.length > 0,
  platform: text => platformTag.test(text) && !secondPlatform.test(text),
  permission: text => /^[a-z][\w.-]*/.test(text),
  supports: text => /^[a-z][\w.]*$/.test(text),
  example: text => text.includes("```"),
  param: () => true,
  see: text => text.length > 0,
  deprecated: () => true,
};
function checkDoc(path, item) {
  const jsdoc = item.jsdoc ?? [];
  for (const tag of jsdoc) {
    const shape = tagShape[tag.name];
    if (!shape || !shape(tag.text.trim())) find("tag-vocabulary", `${path}:@${tag.name}`);
  }
  const keys = jsdoc.filter(tag => tag.name === "supports").map(tag => tag.text.trim());
  if (jsdoc.some(tag => tag.name === "throws" && /^\{?E_NOT_SUPPORTED\b/.test(tag.text.trim())) && !keys.length) find("supports-key", path);
  for (const key of keys) if (!featureKeys.has(key)) find("supports-key", `${path}:${key}`);
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
  const settles = member.signatures.some(signature => /^Promise<|^\w*Task\b/.test(signature.returns));
  if (settles && !member.tags.includes("throws") && !/\bNever rejects\b/.test(member.doc)) find("throws", member.path);
  if (/^get[A-Z]/.test(name)) find("no-get-read", member.path);
  if (member.signatures.some(signature => shapeOf(signature)?.fits === false)) find("argument-shape", member.path);
  if (/^(watch\w*|on[A-Z]\w*)$/.test(name)) {
    const shaped = member.signatures.every(signature =>
      signature.returns === "() => void" && /SignalOptions/.test(signature.parameters.at(-1)?.type ?? ""));
    if (!shaped) find("subscription-shape", member.path);
  }
  for (const signature of member.signatures) {
    for (const parameter of signature.parameters) {
      if (isMutableArray(parameter.type))
        find("readonly-array-param", `${member.path}(${parameter.name})`);
      if (parameter.name === "url" && isBareUrl(parameter.type)) find("url-type", `${member.path}(${parameter.name})`);
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
    if (isMutableArray(field.type)) find("readonly-array-field", path);
    if (field.name === "url" && isBareUrl(field.type)) find("url-type", path);
    if (field.name === "status" && field.type !== "number") {
      const values = [...field.type.matchAll(/"([^"]+)"/g)].map(([, value]) => value);
      if (values.some(value => value !== "ok" && value !== "canceled")) find("status-values", path);
    }
  }
}

const known = new Set(members.map(member => member.path));
for (const member of full.members) if (!known.has(member.path) && !member.summary) find("summary", member.path);
for (const [type, fields] of full.types) {
  if (types.has(type)) continue;
  for (const field of fields) if (!isGuard(field) && !field.summary) find("summary", `${type}.${field.name}`);
}
const named = [...full.typeInfo].filter(([name]) => !/[.(]/.test(name)).concat([...full.aliases]);
for (const [name, info] of named) if (!info.summary) find("type-summary", name);
for (const namespace of members.filter(member => member.path.split(".").length === 2)) {
  const under = members.filter(member => member.path === namespace.path || member.path.startsWith(`${namespace.path}.`));
  if (!under.some(member => member.tags.includes("example"))) find("namespace-example", namespace.path);
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
].join("\n").replace(/\r\n?/g, "\n");

// A `@throws` code must be one `LxErrorCode` names, or `e.code === …` on it would not type-check.
const declaredCodes = new Set(readFileSync(resolve(packageDir, "src/error.ts"), "utf8").match(/LX_ERROR_CODES = \[([^]*?)\]/)[1].match(/E_[A-Z_]+/g));
for (const file of ["src/generated/logic.ts", "src/page.ts", "src/index.ts"]) {
  for (const [, code] of readFileSync(resolve(packageDir, file), "utf8").matchAll(/@throws \{(E_[A-Z_]+)\}/g)) {
    if (!declaredCodes.has(code)) find("error-code", `${file}:${code}`);
  }
}

// Literals are read from the declarations, where a union alias such as `HostOs` is spelled out.
for (const line of readFileSync(resolve(packageDir, "src/generated/logic.ts"), "utf8").split("\n")) {
  // `LxErrorReason` names are camelCase, like the property an author compares them on.
  if (/^\s*(\*|\/\/|\/\*)/.test(line) || /Brand\]:/.test(line) || /^export type LxErrorReason =/.test(line)) continue;
  for (const [, literal] of line.matchAll(/'([^'\s]+)'/g)) {
    if (/^[A-Z]+$/.test(literal) || /^[a-z0-9]+(_[a-z0-9]+)*$/.test(literal) || /:\/\/|\$\{/.test(literal)) continue;
    if (/^[a-z][\w.]*\.\w/.test(literal) || /^on[A-Z]\w*$|^[a-z]+$/.test(literal)) continue;
    find("literal-case", `logic.ts:'${literal}'`);
  }
}

const current = [...new Set(findings)].filter(finding => !only || only.has(finding.split(" ")[0])).sort();
if (write) {
  const byRule = Object.fromEntries(Object.keys(rules).map(rule => [rule, current.filter(f => f.startsWith(`${rule} `)).map(f => f.slice(rule.length + 1))]));
  writeFileSync(baselinePath, `${JSON.stringify(byRule, null, 2)}\n`);
  writeFileSync(tablePath, table);
  console.log(`wrote ${tablePath} and the baseline (${current.length} findings)`);
  process.exit(0);
}

const baseline = JSON.parse(readFileSync(baselinePath, "utf8"));
const allowed = new Set(Object.entries(baseline).filter(([rule]) => !only || only.has(rule)).flatMap(([rule, paths]) => paths.map(path => `${rule} ${path}`)));
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
if (!only && readFileSync(tablePath, "utf8") !== table) {
  failed = true;
  console.error("docs/internal/logic-api-members.md is stale; run `npm run gen:api`");
}
if (failed) process.exit(1);
const covered = members.length + [...types.values()].flat().filter(field => !isGuard(field)).length;
console.log(`API style: ${members.length} members, ${types.size} option and result types; ${current.length} baseline findings (${current.filter(f => f.startsWith("summary ")).length} without a summary of ${covered}).`);
