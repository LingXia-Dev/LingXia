import type { Lx, Storage, TypedStorage } from "../src/index.js";

declare const lx: Lx;
declare const store: Storage;

interface Draft {
  title: string;
}

async function assertedValueRequiresAbsenceCheck(): Promise<string> {
  const draft = await store.get<Draft>("draft");
  // @ts-expect-error a missing key resolves undefined, so absence must be handled
  draft.title;
  return draft?.title ?? "";
}

async function unannotatedValueStaysUnknown(): Promise<void> {
  const draft = await store.get("draft");
  // @ts-expect-error the type parameter defaults to unknown
  draft.title;
}

async function listResolvesAnArray(): Promise<number> {
  const keys = await store.list("draft:");
  return keys.filter((key) => key.length > 0).length;
}

interface TodoSchema {
  "todo:todos": Draft[];
  "todo:filter": string;
}

declare const typed: TypedStorage<TodoSchema>;

async function schemaPinsKeysAndValues(): Promise<Draft[] | undefined> {
  const todos = await typed.get("todo:todos");
  await typed.set("todo:filter", "open");
  // @ts-expect-error schema keys are closed
  await typed.get("draft");
  // @ts-expect-error value must match the schema
  await typed.set("todo:filter", 1);
  return todos;
}

declare const unionKey: keyof TodoSchema;
declare const unionValue: TodoSchema[keyof TodoSchema];

async function schemaKeepsUnionKeysCorrelated(): Promise<void> {
  // @ts-expect-error independently unioned keys and values are not correlated
  await typed.set(unionKey, unionValue);
}

async function getStorageAcceptsASchema(): Promise<TypedStorage<TodoSchema>> {
  return lx.getStorage<TodoSchema>();
}

async function getStorageWithoutASchemaStaysUntyped(): Promise<void> {
  const untyped = lx.getStorage();
  await untyped.get<Draft>("draft");
  await untyped.set("outside:any-schema", 1);
}

export type StorageTypingGate = [
  typeof assertedValueRequiresAbsenceCheck,
  typeof unannotatedValueStaysUnknown,
  typeof listResolvesAnArray,
  typeof schemaPinsKeysAndValues,
  typeof schemaKeepsUnionKeysCorrelated,
  typeof getStorageAcceptsASchema,
  typeof getStorageWithoutASchemaStaysUntyped,
];
