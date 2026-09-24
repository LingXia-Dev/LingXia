/**
 * OpenAPI contract checks for `lxdev test --openapi`: find the operation a
 * Logic `fetch` hit, and validate its JSON response against the documented
 * schema. lxdev reads and parses the files; this module only sees JSON.
 */
import type { NetworkResponseRecord } from "@lingxia/types/automation";
import {
  formatIssues,
  resolvePointer,
  validateSchema,
  type Dialect,
  type SchemaIssue,
} from "./schema.js";
import type { ContractIssue, OpenApiSummary } from "./types.js";

export interface OpenApiSource {
  /** The file name, as lxdev read it. */
  name: string;
  doc: unknown;
}

/** `toMatchSchema` target: a component schema name, a `#/…` ref, or `{ ref, document? }`. */
export type SchemaTarget = string | { ref: string; document?: string };

interface Segment {
  literal?: string;
}

interface Operation {
  doc: LoadedDocument;
  method: string;
  template: string;
  segments: Segment[];
  literals: number;
  bases: string[];
  operationId?: string;
  responses: Record<string, unknown>;
  pointer: string;
}

interface LoadedDocument {
  name: string;
  root: Record<string, unknown>;
  dialect: Dialect;
  version: string;
  title?: string;
  operations: number;
}

const METHODS = ["get", "put", "post", "delete", "options", "head", "patch", "trace"];
const JSON_TYPE = /^\s*(application|text)\/([^;\s]*\+)?json\s*(;|$)/i;

/** The outcome of one captured response. */
export type ContractCheck =
  | { kind: "unmatched"; method: string; path: string }
  | { kind: "undocumented"; operation: string; status: number }
  | { kind: "skipped"; operation: string; status: number; reason: "no_schema" | "not_json" | "empty" | "truncated" }
  | { kind: "valid"; operation: string; status: number; schema: string }
  | { kind: "invalid"; operation: string; status: number; schema: string; issues: SchemaIssue[] };

export class OpenApiIndex {
  readonly documents: LoadedDocument[];
  private readonly operations: Operation[] = [];

  constructor(sources: readonly OpenApiSource[]) {
    this.documents = sources.map((source) => loadDocument(source));
    for (const doc of this.documents) {
      const paths = doc.root.paths;
      if (!isObject(paths)) continue;
      const docBases = basePaths(doc.root.servers);
      for (const [template, rawItem] of Object.entries(paths)) {
        const item = deref(doc.root, rawItem);
        if (!isObject(item)) continue;
        const bases = Array.isArray(item.servers) ? basePaths(item.servers) : docBases;
        const segments = splitPath(template).map((part) =>
          /^\{[^}]+\}$/.test(part) ? {} : { literal: part });
        for (const method of METHODS) {
          const operation = item[method];
          if (!isObject(operation)) continue;
          doc.operations += 1;
          const opBases = Array.isArray(operation.servers) ? basePaths(operation.servers) : bases;
          this.operations.push({
            doc,
            method: method.toUpperCase(),
            template,
            segments,
            literals: segments.filter((segment) => segment.literal !== undefined).length,
            bases: opBases,
            operationId: typeof operation.operationId === "string" ? operation.operationId : undefined,
            responses: isObject(operation.responses) ? operation.responses : {},
            pointer: `#/paths/${escapePointer(template)}/${method}`,
          });
        }
      }
    }
  }

  /**
   * The operation `method url` hits: server base path, then the path
   * template (a `{param}` is one non-empty segment). Hosts are not compared,
   * so the same contract checks staging and production. A concrete path
   * beats a templated one.
   */
  match(method: string, url: string): Operation | undefined {
    const path = pathOf(url);
    let best: Operation | undefined;
    for (const operation of this.operations) {
      if (operation.method !== method.toUpperCase()) continue;
      if (!operation.bases.some((base) => templateMatches(operation.segments, stripBase(path, base)))) continue;
      if (!best || operation.literals > best.literals) best = operation;
    }
    return best;
  }

  /** Check one captured response against the contract. */
  check(record: Pick<NetworkResponseRecord, "method" | "url" | "status" | "contentType" | "body" | "bodyTruncated">): ContractCheck {
    const operation = this.match(record.method, record.url);
    if (!operation) return { kind: "unmatched", method: record.method.toUpperCase(), path: pathOf(record.url) };
    const label = `${operation.method} ${operation.template}`;
    const found = responseFor(operation, record.status);
    if (!found) return { kind: "undocumented", operation: label, status: record.status };
    const media = mediaFor(operation.doc.root, found.response, record.contentType);
    if (!media) {
      return { kind: "skipped", operation: label, status: record.status, reason: record.contentType && !JSON_TYPE.test(record.contentType) ? "not_json" : "no_schema" };
    }
    if (record.body === null || record.body === "") {
      return { kind: "skipped", operation: label, status: record.status, reason: "empty" };
    }
    if (record.bodyTruncated) return { kind: "skipped", operation: label, status: record.status, reason: "truncated" };
    let value: unknown;
    try {
      value = JSON.parse(record.body);
    } catch (error) {
      return {
        kind: "invalid",
        operation: label,
        status: record.status,
        schema: media.pointer,
        issues: [{ path: "", message: `the body is not JSON: ${(error as Error).message}`, schema: media.pointer }],
      };
    }
    const issues = validateSchema(value, media.schema, media.pointer, {
      root: operation.doc.root,
      dialect: operation.doc.dialect,
      direction: "response",
    });
    return issues.length === 0
      ? { kind: "valid", operation: label, status: record.status, schema: media.pointer }
      : { kind: "invalid", operation: label, status: record.status, schema: media.pointer, issues };
  }

  /** Find a `toMatchSchema` target, or throw saying what exists. */
  resolveTarget(target: SchemaTarget): { schema: unknown; pointer: string; doc: LoadedDocument } {
    const ref = typeof target === "string"
      ? (target.startsWith("#") ? target : `#/components/schemas/${escapePointer(target)}`)
      : target.ref;
    const documentName = typeof target === "string" ? undefined : target.document;
    if (typeof ref !== "string" || !ref.startsWith("#")) {
      throw new TypeError("toMatchSchema takes a schema name, a '#/…' ref, or { ref: '#/…', document? }");
    }
    const docs = documentName === undefined
      ? this.documents
      : this.documents.filter((doc) => doc.name === documentName || doc.name.endsWith(`/${documentName}`));
    if (documentName !== undefined && docs.length === 0) {
      throw new Error(`toMatchSchema: no --openapi document named ${JSON.stringify(documentName)} (loaded: ${this.documents.map((doc) => doc.name).join(", ")})`);
    }
    const hits = docs.filter((doc) => resolvePointer(doc.root, ref) !== undefined);
    if (hits.length === 0) {
      const known = this.documents.flatMap((doc) => {
        const schemas = isObject(doc.root.components) && isObject(doc.root.components.schemas)
          ? Object.keys(doc.root.components.schemas) : [];
        return schemas;
      });
      throw new Error(
        `toMatchSchema: ${ref} is not in ${docs.map((doc) => doc.name).join(", ")}` +
          (known.length > 0 ? ` (component schemas: ${known.slice(0, 20).join(", ")}${known.length > 20 ? ", …" : ""})` : ""),
      );
    }
    if (hits.length > 1) {
      throw new Error(`toMatchSchema: ${ref} is defined in ${hits.map((doc) => doc.name).join(" and ")}; pass { ref, document }`);
    }
    const doc = hits[0]!;
    return { schema: resolvePointer(doc.root, ref), pointer: ref, doc };
  }

  /** Validate `value` against a target; empty when it matches. */
  validate(value: unknown, target: SchemaTarget): { issues: SchemaIssue[]; pointer: string; document: string } {
    const { schema, pointer, doc } = this.resolveTarget(target);
    return {
      issues: validateSchema(value, schema, pointer, { root: doc.root, dialect: doc.dialect, direction: "response" }),
      pointer,
      document: doc.name,
    };
  }
}

function loadDocument(source: OpenApiSource): LoadedDocument {
  if (!isObject(source.doc)) throw new Error(`--openapi ${source.name}: not an OpenAPI document (expected an object)`);
  const version = source.doc.openapi;
  if (typeof version !== "string" || !/^3\.[01]\./.test(version)) {
    throw new Error(`--openapi ${source.name}: openapi ${JSON.stringify(version)} is not supported; use OpenAPI 3.0 or 3.1`);
  }
  const info = isObject(source.doc.info) ? source.doc.info : {};
  return {
    name: source.name,
    root: source.doc,
    dialect: version.startsWith("3.1") ? "3.1" : "3.0",
    version,
    title: typeof info.title === "string" ? info.title : undefined,
    operations: 0,
  };
}

/** Server URL path prefixes, with `{variables}` at their defaults. */
function basePaths(servers: unknown): string[] {
  const list = Array.isArray(servers) && servers.length > 0 ? servers : [{ url: "/" }];
  const bases = new Set<string>();
  for (const server of list) {
    if (!isObject(server) || typeof server.url !== "string") continue;
    const variables = isObject(server.variables) ? server.variables : {};
    const url = server.url.replace(/\{([^}]+)\}/g, (_, name: string) => {
      const variable = variables[name];
      return isObject(variable) && typeof variable.default === "string" ? variable.default : "";
    });
    bases.add(normalizePath(pathOf(url)));
  }
  return [...bases];
}

/** The path of an absolute or relative URL, without query or fragment. */
export function pathOf(url: string): string {
  const cut = url.search(/[?#]/);
  const bare = cut >= 0 ? url.slice(0, cut) : url;
  const match = /^[a-z][a-z0-9+.-]*:\/\/[^/]*(.*)$/i.exec(bare);
  const path = match ? match[1]! : bare;
  return path.startsWith("/") ? path : `/${path}`;
}

function normalizePath(path: string): string {
  const trimmed = path.replace(/\/+$/, "");
  return trimmed === "" ? "" : trimmed;
}

function stripBase(path: string, base: string): string | undefined {
  if (base === "") return path;
  if (path === base) return "/";
  return path.startsWith(`${base}/`) ? path.slice(base.length) : undefined;
}

function splitPath(path: string): string[] {
  return path.split("/").filter((part) => part.length > 0);
}

/** `true` when `path` fits the template; a `{param}` takes one non-empty segment. */
export function templateMatches(segments: readonly Segment[], path: string | undefined): boolean {
  if (path === undefined) return false;
  const parts = splitPath(path);
  if (parts.length !== segments.length) return false;
  return segments.every((segment, index) => {
    if (segment.literal === undefined) return true;
    const part = parts[index]!;
    if (part === segment.literal) return true;
    try {
      return decodeURIComponent(part) === segment.literal;
    } catch {
      return false;
    }
  });
}

/** Compile a path template for tests and tools. */
export function compileTemplate(template: string): Segment[] {
  return splitPath(template).map((part) => (/^\{[^}]+\}$/.test(part) ? {} : { literal: part }));
}

/** The documented response for `status`: exact, then `4XX`, then `default`. */
function responseFor(operation: Operation, status: number): { response: Record<string, unknown> } | undefined {
  const responses = operation.responses;
  const range = `${Math.floor(status / 100)}XX`;
  const key = Object.keys(responses).find((name) => name === String(status))
    ?? Object.keys(responses).find((name) => name.toUpperCase() === range)
    ?? (Object.prototype.hasOwnProperty.call(responses, "default") ? "default" : undefined);
  if (key === undefined) return undefined;
  const response = deref(operation.doc.root, responses[key]);
  return isObject(response) ? { response } : undefined;
}

/** The JSON media type entry that describes a response of `contentType`. */
function mediaFor(
  root: Record<string, unknown>,
  response: Record<string, unknown>,
  contentType: string | null,
): { schema: unknown; pointer: string } | undefined {
  const content = response.content;
  if (!isObject(content)) return undefined;
  const actual = (contentType ?? "").split(";")[0]!.trim().toLowerCase();
  if (actual !== "" && !JSON_TYPE.test(actual)) return undefined;
  const keys = Object.keys(content);
  const key = keys.find((name) => name.split(";")[0]!.trim().toLowerCase() === actual)
    ?? keys.find((name) => name.toLowerCase().startsWith("application/json"))
    ?? keys.find((name) => JSON_TYPE.test(name))
    ?? keys.find((name) => name === "*/*" || name === "application/*");
  if (key === undefined) return undefined;
  const media = content[key];
  if (!isObject(media) || media.schema === undefined) return undefined;
  const schema = media.schema;
  const pointer = isObject(schema) && typeof schema.$ref === "string" && Object.keys(schema).length === 1
    ? schema.$ref
    : `${responsePointer(root, response)}/content/${escapePointer(key)}/schema`;
  return { schema, pointer };
}

/** Best-effort location of a response object, for messages only. */
function responsePointer(root: Record<string, unknown>, response: Record<string, unknown>): string {
  const components = isObject(root.components) && isObject(root.components.responses) ? root.components.responses : {};
  for (const [name, candidate] of Object.entries(components)) {
    if (candidate === response) return `#/components/responses/${escapePointer(name)}`;
  }
  return "response";
}

function deref(root: unknown, node: unknown): unknown {
  let current = node;
  for (let hop = 0; hop < 32 && isObject(current) && typeof current.$ref === "string"; hop += 1) {
    current = resolvePointer(root, current.$ref);
  }
  return current;
}

function escapePointer(segment: string): string {
  return segment.replace(/~/g, "~0").replace(/\//g, "~1");
}

function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

// ------------------------------ run state ------------------------------

let active: OpenApiIndex | undefined;

/** The run's contract, for `toMatchSchema`; `undefined` without `--openapi`. */
export function activeOpenApi(): OpenApiIndex | undefined {
  return active;
}

export function setActiveOpenApi(index: OpenApiIndex | undefined): void {
  active = index;
}

/** `control.openapi`: a JSON list of `{ name, doc }` lxdev parsed from the files. */
export function parseOpenApiControl(raw: string | undefined): OpenApiIndex | undefined {
  if (raw === undefined) return undefined;
  const parsed: unknown = JSON.parse(raw);
  if (!Array.isArray(parsed) || parsed.length === 0) throw new Error("openapi control must be a non-empty JSON list");
  return new OpenApiIndex(parsed.map((entry, index) => {
    const record = (isObject(entry) ? entry : {}) as { name?: unknown; doc?: unknown };
    return { name: typeof record.name === "string" ? record.name : `openapi-${index + 1}`, doc: record.doc };
  }));
}

const MAX_WARNINGS = 100;
const MAX_UNMATCHED = 50;

/** Accumulates the run's `openapi` report section. */
export class ContractLedger {
  readonly summary: OpenApiSummary;

  constructor(readonly index: OpenApiIndex) {
    this.summary = {
      documents: index.documents.map((doc) => ({
        name: doc.name,
        version: doc.version,
        ...(doc.title ? { title: doc.title } : {}),
        operations: doc.operations,
      })),
      capture: "ok",
      responses: 0,
      validated: 0,
      routed: { validated: 0, failed: 0 },
      network: { validated: 0, mismatched: 0 },
      skipped: { no_schema: 0, not_json: 0, empty: 0, truncated: 0 },
      undocumented: [],
      unmatched: [],
      warnings: [],
    };
  }

  /**
   * Check one spec's responses. Mismatches in routed responses (`route`,
   * `patch`) are the spec's violations; the real server's are warnings.
   */
  checkSpec(caseId: string, records: readonly NetworkResponseRecord[]): { violations: ContractIssue[]; warnings: ContractIssue[]; checked: number } {
    const violations: ContractIssue[] = [];
    const warnings: ContractIssue[] = [];
    for (const record of records) {
      this.summary.responses += 1;
      const outcome = this.index.check(record);
      const routed = record.source !== "network";
      switch (outcome.kind) {
        case "unmatched": {
          const found = this.summary.unmatched.find((entry) => entry.method === outcome.method && entry.path === outcome.path);
          if (found) found.count += 1;
          else if (this.summary.unmatched.length < MAX_UNMATCHED) {
            this.summary.unmatched.push({ method: outcome.method, path: outcome.path, count: 1 });
          }
          break;
        }
        case "undocumented": {
          const found = this.summary.undocumented.find((entry) =>
            entry.operation === outcome.operation && entry.status === outcome.status && entry.source === record.source);
          if (found) found.count += 1;
          else this.summary.undocumented.push({ operation: outcome.operation, status: outcome.status, source: record.source, count: 1 });
          break;
        }
        case "skipped":
          this.summary.skipped[outcome.reason] += 1;
          break;
        case "valid":
          this.summary.validated += 1;
          if (routed) this.summary.routed.validated += 1;
          else this.summary.network.validated += 1;
          break;
        case "invalid": {
          this.summary.validated += 1;
          const issue: ContractIssue = {
            case: caseId,
            source: record.source,
            method: record.method,
            url: record.url,
            status: outcome.status,
            operation: outcome.operation,
            schema: outcome.schema,
            ...(record.pattern ? { pattern: record.pattern } : {}),
            issues: outcome.issues.map(({ path, message, schema }) => ({ path, message, schema })),
          };
          if (routed) {
            this.summary.routed.validated += 1;
            this.summary.routed.failed += 1;
            violations.push(issue);
          } else {
            this.summary.network.validated += 1;
            this.summary.network.mismatched += 1;
            warnings.push(issue);
            if (this.summary.warnings.length < MAX_WARNINGS) this.summary.warnings.push(issue);
          }
          break;
        }
      }
    }
    return { violations, warnings, checked: records.length };
  }
}

/** The spec failure a routed contract violation produces. */
export class ContractError extends Error {
  override readonly name = "ContractError";
  readonly code = "E_OPENAPI_CONTRACT";

  constructor(violations: readonly ContractIssue[]) {
    super(describeViolations(violations));
  }
}

export function describeViolations(violations: readonly ContractIssue[]): string {
  const lines = violations.slice(0, 5).map((violation) => {
    const by = violation.source === "patch" ? "patched by" : "fulfilled by";
    const route = violation.pattern ? ` (${by} route ${violation.pattern})` : "";
    return `${violation.operation} → ${violation.status}${route}: the response does not match ${violation.schema}\n${formatIssues(violation.issues)}`;
  });
  const more = violations.length > 5 ? `\n…and ${violations.length - 5} more` : "";
  const head = violations.length === 1
    ? "A routed response breaks the OpenAPI contract:"
    : `${violations.length} routed responses break the OpenAPI contract:`;
  return `${head}\n${lines.join("\n")}${more}`;
}
