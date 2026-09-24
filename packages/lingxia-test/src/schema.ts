/**
 * A JSON Schema validator for the schema objects of OpenAPI 3.0 and 3.1.
 *
 * Dependency-free on purpose: it runs in the test worker on the device,
 * where `expect(...).toMatchSchema()` has to answer synchronously. It covers
 * what API contracts use — `$ref`, `type` (3.1 arrays, 3.0 `nullable`),
 * `enum`/`const`, `required`, `properties`, `additionalProperties`,
 * `patternProperties`, `items`/`prefixItems`, `allOf`/`anyOf`/`oneOf`/`not`
 * (with `discriminator`), `if`/`then`/`else`, and the numeric, string and
 * size bounds. `format` is an annotation: it is never asserted.
 */
import { isEqual } from "./equal.js";

export type Dialect = "3.0" | "3.1";

/** One way the value breaks the schema. */
export interface SchemaIssue {
  /** JSON pointer into the value (`""` is the value itself). */
  path: string;
  message: string;
  /** Where in the document the failing schema lives. */
  schema: string;
  /** Set for a `type` mismatch, which also decides the closest alternative. */
  keyword?: "type";
}

export interface SchemaContext {
  /** The whole document `$ref` pointers resolve against. */
  root: unknown;
  dialect: Dialect;
  /**
   * `response`: a `writeOnly` property is never required. `request`: a
   * `readOnly` one is never required.
   */
  direction?: "response" | "request";
}

const MAX_ISSUES = 20;
const MAX_REF_HOPS = 64;
/** No `$ref` entered yet at the current value. */
const NO_REFS: ReadonlySet<string> = new Set();

type Schema = Record<string, unknown>;

/** Validate `value`; an empty list means it matches. */
export function validateSchema(
  value: unknown,
  schema: unknown,
  schemaPath: string,
  context: SchemaContext,
): SchemaIssue[] {
  const issues = new Validator(context).check(value, schema, "", schemaPath, NO_REFS);
  return issues.slice(0, MAX_ISSUES);
}

/** Resolve a local `#/…` pointer in `root`; `undefined` when it names nothing. */
export function resolvePointer(root: unknown, ref: string): unknown {
  if (ref === "#" || ref === "") return root;
  if (!ref.startsWith("#/")) return undefined;
  let node: unknown = root;
  for (const raw of ref.slice(2).split("/")) {
    let key: string;
    try {
      key = decodeURIComponent(raw).replace(/~1/g, "/").replace(/~0/g, "~");
    } catch {
      return undefined;
    }
    if (node === null || typeof node !== "object" || !Object.prototype.hasOwnProperty.call(node, key)) {
      return undefined;
    }
    node = (node as Record<string, unknown>)[key];
  }
  return node;
}

export function pointerSegment(key: string | number): string {
  return String(key).replace(/~/g, "~0").replace(/\//g, "~1");
}

/** `path: message`, one per line, for an assertion or a report. */
export function formatIssues(issues: readonly SchemaIssue[]): string {
  return issues.map((issue) => `  at ${issue.path || "/"}: ${issue.message} (${issue.schema})`).join("\n");
}

class Validator {
  private readonly direction: "response" | "request";

  /** `(value, $ref)` results, issue paths relative to the value. */
  private readonly memo = new Map<unknown, Map<string, SchemaIssue[]>>();
  /** Cycle issues reported so far; a result that saw one is not memoized. */
  private cycles = 0;

  constructor(private readonly context: SchemaContext) {
    this.direction = context.direction ?? "response";
  }

  /**
   * `value` against the schema `ref` names, memoized: the same value meets
   * the same `$ref` again through `anyOf`/`oneOf` branches, and checking it
   * afresh each time grows exponentially with nesting.
   */
  private checkRef(value: unknown, target: unknown, ref: string, refs: ReadonlySet<string>): SchemaIssue[] {
    let byRef = this.memo.get(value);
    const cached = byRef?.get(ref);
    if (cached) return cached;
    const cyclesBefore = this.cycles;
    const entered = new Set(refs);
    entered.add(ref);
    const issues = this.check(value, target, "", ref, entered);
    // A cycle report depends on the refs entered on the way here.
    if (this.cycles === cyclesBefore) {
      if (!byRef) {
        byRef = new Map();
        this.memo.set(value, byRef);
      }
      byRef.set(ref, issues);
    }
    return issues;
  }

  /**
   * `refs`: the `$ref`s already entered for this same value. A `$ref` seen
   * again before the value changes is a cycle that never reaches data; one
   * seen again deeper in the value is ordinary recursive data, however deep.
   */
  check(value: unknown, schema: unknown, path: string, at: string, refs: ReadonlySet<string>): SchemaIssue[] {
    if (schema === true || schema === undefined) return [];
    if (schema === false) return [{ path, message: "no value is allowed here", schema: at }];
    if (schema === null || typeof schema !== "object" || Array.isArray(schema)) {
      return [{ path, message: "the contract's schema here is not an object", schema: at }];
    }
    const node = schema as Schema;
    if (typeof node.$ref === "string") {
      const ref = node.$ref;
      if (refs.has(ref)) {
        this.cycles += 1;
        return [{ path, message: `$ref ${ref} refers back to itself without matching any value: circular`, schema: at }];
      }
      const target = resolvePointer(this.context.root, ref);
      if (target === undefined) {
        return [{ path, message: `$ref ${ref} does not resolve (only local #/… references are supported)`, schema: at }];
      }
      const viaRef = this.checkRef(value, target, ref, refs).map((issue) => ({ ...issue, path: path + issue.path }));
      // 3.0 ignores a $ref's siblings; 3.1 applies them too.
      if (this.context.dialect === "3.0") return viaRef;
      const siblings = { ...node };
      delete siblings.$ref;
      return [...viaRef, ...this.check(value, siblings, path, at, refs)];
    }

    // 3.0 `nullable` widens `type` to null; it does not relax anything else,
    // but a null value has nothing else to check.
    if (value === null && this.context.dialect === "3.0" && node.nullable === true) return [];

    const typeIssue = this.checkType(value, node, path, at);
    if (typeIssue) return [typeIssue];

    const issues: SchemaIssue[] = [];
    const push = (message: string) => issues.push({ path, message, schema: at });

    if (Array.isArray(node.enum) && !node.enum.some((option) => isEqual(option, value))) {
      push(`expected one of ${preview(node.enum)}, got ${preview(value)}`);
    }
    if ("const" in node && !isEqual(node.const, value)) {
      push(`expected ${preview(node.const)}, got ${preview(value)}`);
    }

    if (typeof value === "string") this.checkString(value, node, push);
    if (typeof value === "number") this.checkNumber(value, node, push);
    if (Array.isArray(value)) issues.push(...this.checkArray(value, node, path, at));
    else if (value !== null && typeof value === "object") {
      issues.push(...this.checkObject(value as Record<string, unknown>, node, path, at));
    }

    issues.push(...this.checkCombinators(value, node, path, at, refs));
    return issues;
  }

  private checkType(value: unknown, node: Schema, path: string, at: string): SchemaIssue | undefined {
    const declared = node.type;
    if (declared === undefined) return undefined;
    const types = Array.isArray(declared) ? declared.map(String) : [String(declared)];
    const accepted = [...types];
    if (this.context.dialect === "3.0" && node.nullable === true) accepted.push("null");
    if (accepted.some((type) => hasType(value, type))) return undefined;
    return { path, message: `expected ${accepted.join(" or ")}, got ${describe(value)}`, schema: at, keyword: "type" };
  }

  private checkString(value: string, node: Schema, push: (message: string) => void): void {
    const length = [...value].length;
    if (typeof node.minLength === "number" && length < node.minLength) {
      push(`expected at least ${node.minLength} characters, got ${length}`);
    }
    if (typeof node.maxLength === "number" && length > node.maxLength) {
      push(`expected at most ${node.maxLength} characters, got ${length}`);
    }
    if (typeof node.pattern === "string") {
      const pattern = compile(node.pattern);
      if (pattern && !pattern.test(value)) push(`expected to match /${node.pattern}/, got ${preview(value)}`);
    }
  }

  private checkNumber(value: number, node: Schema, push: (message: string) => void): void {
    const exclusiveMin = node.exclusiveMinimum;
    const exclusiveMax = node.exclusiveMaximum;
    if (typeof node.minimum === "number") {
      // 3.0: `exclusiveMinimum: true` turns `minimum` exclusive.
      if (exclusiveMin === true ? value <= node.minimum : value < node.minimum) {
        push(`expected ${exclusiveMin === true ? ">" : ">="} ${node.minimum}, got ${value}`);
      }
    }
    if (typeof node.maximum === "number") {
      if (exclusiveMax === true ? value >= node.maximum : value > node.maximum) {
        push(`expected ${exclusiveMax === true ? "<" : "<="} ${node.maximum}, got ${value}`);
      }
    }
    // 3.1: the exclusive bounds are numbers of their own.
    if (typeof exclusiveMin === "number" && value <= exclusiveMin) push(`expected > ${exclusiveMin}, got ${value}`);
    if (typeof exclusiveMax === "number" && value >= exclusiveMax) push(`expected < ${exclusiveMax}, got ${value}`);
    if (typeof node.multipleOf === "number" && node.multipleOf > 0) {
      const quotient = value / node.multipleOf;
      if (Math.abs(quotient - Math.round(quotient)) > 1e-9) push(`expected a multiple of ${node.multipleOf}, got ${value}`);
    }
  }

  private checkArray(value: unknown[], node: Schema, path: string, at: string): SchemaIssue[] {
    const issues: SchemaIssue[] = [];
    const push = (message: string) => issues.push({ path, message, schema: at });
    if (typeof node.minItems === "number" && value.length < node.minItems) {
      push(`expected at least ${node.minItems} items, got ${value.length}`);
    }
    if (typeof node.maxItems === "number" && value.length > node.maxItems) {
      push(`expected at most ${node.maxItems} items, got ${value.length}`);
    }
    if (node.uniqueItems === true) {
      for (let i = 1; i < value.length; i += 1) {
        const duplicate = value.slice(0, i).findIndex((earlier) => isEqual(earlier, value[i]));
        if (duplicate >= 0) {
          push(`expected unique items, but items ${duplicate} and ${i} are equal`);
          break;
        }
      }
    }
    const prefix = Array.isArray(node.prefixItems) ? node.prefixItems : [];
    for (let i = 0; i < value.length && issues.length < MAX_ISSUES; i += 1) {
      const itemPath = `${path}/${i}`;
      if (i < prefix.length) {
        issues.push(...this.check(value[i], prefix[i], itemPath, `${at}/prefixItems/${i}`, NO_REFS));
      } else if (node.items !== undefined && !Array.isArray(node.items)) {
        issues.push(...this.check(value[i], node.items, itemPath, `${at}/items`, NO_REFS));
      }
    }
    if (node.contains !== undefined) {
      const hits = value.filter((item) => this.check(item, node.contains, path, `${at}/contains`, NO_REFS).length === 0).length;
      const min = typeof node.minContains === "number" ? node.minContains : 1;
      if (hits < min) push(`expected at least ${min} item(s) matching \`contains\`, got ${hits}`);
      if (typeof node.maxContains === "number" && hits > node.maxContains) {
        push(`expected at most ${node.maxContains} item(s) matching \`contains\`, got ${hits}`);
      }
    }
    return issues;
  }

  private checkObject(
    value: Record<string, unknown>,
    node: Schema,
    path: string,
    at: string,
  ): SchemaIssue[] {
    const issues: SchemaIssue[] = [];
    const push = (message: string) => issues.push({ path, message, schema: at });
    const properties = (isObject(node.properties) ? node.properties : {}) as Record<string, unknown>;
    const keys = Object.keys(value);
    if (typeof node.minProperties === "number" && keys.length < node.minProperties) {
      push(`expected at least ${node.minProperties} properties, got ${keys.length}`);
    }
    if (typeof node.maxProperties === "number" && keys.length > node.maxProperties) {
      push(`expected at most ${node.maxProperties} properties, got ${keys.length}`);
    }
    if (Array.isArray(node.required)) {
      for (const name of node.required) {
        if (typeof name !== "string" || Object.prototype.hasOwnProperty.call(value, name)) continue;
        if (this.exemptFromRequired(properties[name])) continue;
        push(`missing required property ${JSON.stringify(name)}`);
      }
    }
    if (isObject(node.dependentRequired)) {
      for (const [name, needed] of Object.entries(node.dependentRequired as Record<string, unknown>)) {
        if (!(name in value) || !Array.isArray(needed)) continue;
        for (const other of needed) {
          if (typeof other === "string" && !(other in value)) {
            push(`property ${JSON.stringify(name)} requires ${JSON.stringify(other)}`);
          }
        }
      }
    }
    const patterns = isObject(node.patternProperties)
      ? Object.entries(node.patternProperties as Record<string, unknown>)
        .map(([source, schema]) => ({ source, schema, regex: compile(source) }))
      : [];
    for (const key of keys) {
      if (issues.length >= MAX_ISSUES) break;
      const keyPath = `${path}/${pointerSegment(key)}`;
      let matched = false;
      if (Object.prototype.hasOwnProperty.call(properties, key)) {
        matched = true;
        issues.push(...this.check(value[key], properties[key], keyPath, `${at}/properties/${pointerSegment(key)}`, NO_REFS));
      }
      for (const pattern of patterns) {
        if (!pattern.regex?.test(key)) continue;
        matched = true;
        issues.push(...this.check(value[key], pattern.schema, keyPath, `${at}/patternProperties/${pointerSegment(pattern.source)}`, NO_REFS));
      }
      if (node.propertyNames !== undefined) {
        for (const issue of this.check(key, node.propertyNames, keyPath, `${at}/propertyNames`, NO_REFS)) {
          issues.push({ ...issue, message: `property name ${JSON.stringify(key)}: ${issue.message}` });
        }
      }
      if (matched) continue;
      if (node.additionalProperties === false) {
        issues.push({ path: keyPath, message: `unexpected property ${JSON.stringify(key)} (additionalProperties: false)`, schema: at });
      } else if (isObject(node.additionalProperties)) {
        issues.push(...this.check(value[key], node.additionalProperties, keyPath, `${at}/additionalProperties`, NO_REFS));
      }
    }
    return issues;
  }

  /** `writeOnly` properties are not in a response, `readOnly` not in a request. */
  private exemptFromRequired(property: unknown): boolean {
    const resolved = this.deref(property);
    if (!isObject(resolved)) return false;
    return this.direction === "response" ? resolved.writeOnly === true : resolved.readOnly === true;
  }

  private deref(schema: unknown): unknown {
    let node = schema;
    for (let hop = 0; hop < MAX_REF_HOPS && isObject(node) && typeof node.$ref === "string"; hop += 1) {
      node = resolvePointer(this.context.root, node.$ref);
    }
    return node;
  }

  private checkCombinators(value: unknown, node: Schema, path: string, at: string, refs: ReadonlySet<string>): SchemaIssue[] {
    const issues: SchemaIssue[] = [];
    if (Array.isArray(node.allOf)) {
      node.allOf.forEach((branch, index) => {
        issues.push(...this.check(value, branch, path, `${at}/allOf/${index}`, refs));
      });
    }
    for (const keyword of ["anyOf", "oneOf"] as const) {
      const branches = node[keyword];
      if (!Array.isArray(branches)) continue;
      const chosen = this.discriminated(value, node, branches, keyword, path, at);
      if (chosen) {
        if ("issue" in chosen) issues.push(chosen.issue);
        else issues.push(...this.check(value, chosen.branch, path, `${at}/${keyword}/${chosen.index}`, refs));
        continue;
      }
      const results = branches.map((branch, index) => this.check(value, branch, path, `${at}/${keyword}/${index}`, refs));
      const passing = results.flatMap((result, index) => (result.length === 0 ? [index] : []));
      if (passing.length === 0) {
        const best = closest(results, path);
        issues.push({
          path,
          message: `matches none of the ${branches.length} ${keyword} alternatives${best ? `; closest is #${best.index}` : ""}`,
          schema: `${at}/${keyword}`,
        });
        if (best) issues.push(...best.issues);
      } else if (keyword === "oneOf" && passing.length > 1) {
        issues.push({
          path,
          message: `matches ${passing.length} oneOf alternatives (#${passing.join(", #")}); exactly one must match`,
          schema: `${at}/oneOf`,
        });
      }
    }
    if (node.not !== undefined && this.check(value, node.not, path, `${at}/not`, refs).length === 0) {
      issues.push({ path, message: "matches the schema under `not`", schema: `${at}/not` });
    }
    if (node.if !== undefined) {
      const holds = this.check(value, node.if, path, `${at}/if`, refs).length === 0;
      const next = holds ? node.then : node.else;
      if (next !== undefined) issues.push(...this.check(value, next, path, `${at}/${holds ? "then" : "else"}`, refs));
    }
    return issues;
  }

  /**
   * With a `discriminator`, the property names the one branch that applies:
   * its errors are the useful ones, not "matches none of 4 alternatives".
   */
  private discriminated(
    value: unknown,
    node: Schema,
    branches: unknown[],
    keyword: string,
    path: string,
    at: string,
  ): { branch: unknown; index: number } | { issue: SchemaIssue } | undefined {
    const discriminator = node.discriminator;
    if (!isObject(discriminator) || typeof discriminator.propertyName !== "string") return undefined;
    if (!isObject(value)) return undefined;
    const name = discriminator.propertyName;
    const tag = value[name];
    if (typeof tag !== "string") {
      return { issue: { path, message: `missing discriminator property ${JSON.stringify(name)}`, schema: `${at}/discriminator` } };
    }
    const mapping = isObject(discriminator.mapping) ? discriminator.mapping as Record<string, unknown> : {};
    const mapped = mapping[tag];
    const target = typeof mapped === "string"
      ? (mapped.startsWith("#") ? mapped : `#/components/schemas/${mapped}`)
      : `#/components/schemas/${tag}`;
    const index = branches.findIndex((branch) => isObject(branch) && branch.$ref === target);
    if (index < 0) {
      const known = [
        ...Object.keys(mapping),
        ...branches.flatMap((branch) =>
          isObject(branch) && typeof branch.$ref === "string" ? [branch.$ref.split("/").pop()!] : []),
      ];
      return {
        issue: {
          path: `${path}/${pointerSegment(name)}`,
          message: `discriminator ${JSON.stringify(tag)} names no ${keyword} alternative (known: ${[...new Set(known)].join(", ")})`,
          schema: `${at}/discriminator`,
        },
      };
    }
    return { branch: branches[index], index };
  }
}

/** The failing branch most likely meant: its type fits, then fewest issues. */
function closest(results: SchemaIssue[][], path: string): { index: number; issues: SchemaIssue[] } | undefined {
  let best: { index: number; issues: SchemaIssue[]; score: number } | undefined;
  results.forEach((issues, index) => {
    const typeMiss = issues.some((issue) => issue.path === path && issue.keyword === "type");
    const score = (typeMiss ? 1000 : 0) + issues.length;
    if (!best || score < best.score) best = { index, issues, score };
  });
  return best && best.score < 1000 ? { index: best.index, issues: best.issues } : undefined;
}

function hasType(value: unknown, type: string): boolean {
  switch (type) {
    case "null": return value === null;
    case "boolean": return typeof value === "boolean";
    case "string": return typeof value === "string";
    case "number": return typeof value === "number" && Number.isFinite(value);
    case "integer": return typeof value === "number" && Number.isInteger(value);
    case "array": return Array.isArray(value);
    case "object": return isObject(value);
    default: return false;
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function describe(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return `array (${value.length} items)`;
  if (typeof value === "object") return "object";
  if (typeof value === "number" && Number.isInteger(value)) return `integer ${value}`;
  return `${typeof value} ${preview(value)}`;
}

function preview(value: unknown): string {
  let text: string;
  try {
    text = JSON.stringify(value) ?? String(value);
  } catch {
    text = String(value);
  }
  return text.length > 80 ? `${text.slice(0, 77)}...` : text;
}

const compiled = new Map<string, RegExp | null>();

/** ECMA-262 is what JSON Schema patterns are; a pattern JS rejects is skipped. */
function compile(source: string): RegExp | null {
  if (!compiled.has(source)) {
    let regex: RegExp | null = null;
    try {
      regex = new RegExp(source, "u");
    } catch {
      try {
        regex = new RegExp(source);
      } catch {
        regex = null;
      }
    }
    compiled.set(source, regex);
  }
  return compiled.get(source) ?? null;
}
