import type { Lx, LxFile } from "../src/index.js";

declare const lx: Lx;

async function representationsHaveConcreteTypes(): Promise<number> {
  const file: LxFile = lx.fs.file("lx://userdata/notes.json");
  const text: string = await file.text();
  const bytes: Uint8Array = await file.bytes();
  const buffer: ArrayBuffer = await file.arrayBuffer();
  const base64: string = await file.base64();
  const json: unknown = await file.json();
  void json;
  return text.length + bytes.byteLength + buffer.byteLength + base64.length;
}

async function writesUseWebDataTypes(): Promise<void> {
  await lx.fs.write("notes.txt", "hello");
  await lx.fs.write("bytes.bin", new Uint8Array([1, 2, 3]));
  await lx.fs.write("bytes.bin", new Uint8Array([1, 2, 3]).buffer, { overwrite: true });
  await lx.fs.write("encoded.bin", "AQID", { encoding: "base64" });
}

// `encoding` says how to read a string, so pairing it with bytes is a mistake
// the runtime rejects — the byte overload is what makes it a compile error.
// @ts-expect-error encoding has no meaning for binary data
void lx.fs.write("bytes.bin", new Uint8Array([1, 2, 3]), { encoding: "base64" });

// @ts-expect-error the manager factory was replaced by the lx.fs namespace
lx.getFileManager;
// @ts-expect-error file representations are methods on LxFile
lx.fs.readTextFile;
// @ts-expect-error unknown string encodings are rejected
void lx.fs.write("notes.txt", "hello", { encoding: "latin1" });

export type ManagedFileGate = [
  typeof representationsHaveConcreteTypes,
  typeof writesUseWebDataTypes,
];
