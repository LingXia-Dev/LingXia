import assert from "node:assert/strict";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

test("lx used in Logic without the import is a type error naming the import", () => {
  const file = fileURLToPath(new URL("./missing-import.ts", import.meta.url));
  const program = ts.createProgram([file], {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.NodeNext, moduleResolution: ts.ModuleResolutionKind.NodeNext,
    strict: true, noEmit: true, skipLibCheck: true, types: ["@lingxia/types/logic-globals"],
  });
  const messages = ts.getPreEmitDiagnostics(program, program.getSourceFile(file))
    .map(diagnostic => ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n"));
  assert.equal(messages.length, 1, messages.join("\n"));
  assert.match(messages[0], /import \{ lx \} from '@lingxia\/logic'/);
});

test("a View program does not see lx at all", () => {
  const file = fileURLToPath(new URL("./missing-import.ts", import.meta.url));
  const program = ts.createProgram([file], {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.NodeNext, moduleResolution: ts.ModuleResolutionKind.NodeNext,
    strict: true, noEmit: true, skipLibCheck: true, types: [],
  });
  const messages = ts.getPreEmitDiagnostics(program, program.getSourceFile(file)).map(d => d.code);
  assert.deepEqual(messages, [2304], "Cannot find name 'lx'");
});
