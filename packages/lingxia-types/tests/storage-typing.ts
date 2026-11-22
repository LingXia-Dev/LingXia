import type { JsonValue, Lx, Storage } from "../src/index.js";

declare const lx: Lx;

interface Draft {
  title: string;
}

const isDraft = (value: unknown): value is Draft =>
  typeof value === "object" && value !== null && typeof (value as { title?: unknown }).title === "string";

async function withoutASchemaValuesAreJson(): Promise<JsonValue | undefined> {
  const value = await lx.storage.get("draft");
  // @ts-expect-error a missing key resolves undefined, so absence must be handled
  value.length;
  return value;
}

async function decodeChecksAndTypesTheValue(): Promise<string> {
  const draft = await lx.storage.get("draft", {
    decode: (value) => {
      if (!isDraft(value)) throw new TypeError("not a draft");
      return value;
    },
  });
  return draft?.title ?? "";
}

async function setTakesJsonOnly(): Promise<void> {
  const draft: Draft = { title: "x" };
  await lx.storage.set("draft", draft);
  await lx.storage.set("count", 1);
  await lx.storage.set("nothing", null);
  // @ts-expect-error a function is not JSON
  await lx.storage.set("k", () => 1);
  // @ts-expect-error a Map is not JSON
  await lx.storage.set("k", new Map());
  // @ts-expect-error a Date is not JSON
  await lx.storage.set("k", { when: new Date() });
}

async function presenceIsABooleanAndNeedsNoValue(): Promise<boolean> {
  return lx.storage.has("draft");
}

async function listResolvesAnArray(): Promise<number> {
  const keys = await lx.storage.list("draft:");
  return keys.filter((key) => key.length > 0).length;
}

async function infoNamesItsUnits(): Promise<number> {
  const { usedBytes, limitBytes, keys } = await lx.storage.info();
  return limitBytes - usedBytes + keys;
}

interface TodoSchema {
  "todo:todos": Draft[];
  "todo:filter": string;
}

declare const typed: Storage<TodoSchema>;

async function schemaPinsKeysAndValues(): Promise<Draft[] | undefined> {
  const todos = await typed.get("todo:todos");
  await typed.set("todo:filter", "open");
  // @ts-expect-error schema keys are closed
  await typed.get("draft");
  // @ts-expect-error value must match the schema
  await typed.set("todo:filter", 1);
  const present: boolean = await typed.has("todo:filter");
  // @ts-expect-error schema keys are closed for presence too
  await typed.has("draft");
  void present;
  return todos;
}

declare const unionKey: keyof TodoSchema;
declare const unionValue: TodoSchema[keyof TodoSchema];

async function schemaKeepsUnionKeysCorrelated(): Promise<void> {
  // @ts-expect-error independently unioned keys and values are not correlated
  await typed.set(unionKey, unionValue);
}

function storageIsAPropertyNotAFactory(): void {
  // @ts-expect-error `lx.storage` is the store itself
  lx.storage();
}

export type StorageTypingGate = [
  typeof withoutASchemaValuesAreJson,
  typeof decodeChecksAndTypesTheValue,
  typeof setTakesJsonOnly,
  typeof presenceIsABooleanAndNeedsNoValue,
  typeof listResolvesAnArray,
  typeof infoNamesItsUnits,
  typeof schemaPinsKeysAndValues,
  typeof schemaKeepsUnionKeysCorrelated,
  typeof storageIsAPropertyNotAFactory,
];
