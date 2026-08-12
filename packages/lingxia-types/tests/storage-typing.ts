import type { Storage } from "../src/index.js";

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

export type StorageTypingGate = [
  typeof assertedValueRequiresAbsenceCheck,
  typeof unannotatedValueStaysUnknown,
  typeof listResolvesAnArray,
];
