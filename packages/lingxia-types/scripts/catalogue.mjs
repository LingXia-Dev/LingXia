// The Logic API as a list of members, read from the declarations the way an editor sees them:
// every path under `lx`, its signature and summary, and the option and result types it reaches.
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const packageDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export function catalogue() {
  const entry = resolve(packageDir, "src/index.ts");
  const program = ts.createProgram([entry], {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.NodeNext, moduleResolution: ts.ModuleResolutionKind.NodeNext,
    strict: true, noEmit: true, skipLibCheck: true, types: [],
  });
  const checker = program.getTypeChecker();
  const lx = checker.resolveName("lx", undefined, ts.SymbolFlags.Value, false);
  if (!lx) throw Error("no global `lx` in the declarations");
  const at = lx.valueDeclaration;
  const members = [];
  const types = new Map();
  const ownDeclaration = symbol => symbol?.declarations?.some(d => !d.getSourceFile().fileName.includes("/typescript/lib/"));
  // The first sentence of the first paragraph.
  const summary = symbol => {
    const paragraph = ts.displayPartsToString(symbol.getDocumentationComment(checker)).split(/\n\s*\n/)[0].replace(/\s+/g, " ").trim();
    return paragraph.match(/^.*?[.!?](?=\s|$)/)?.[0] ?? paragraph;
  };
  const docText = symbol => ts.displayPartsToString(symbol.getDocumentationComment(checker));
  const tags = symbol => symbol.getJsDocTags(checker).map(tag => tag.name);
  const unqualified = value => value.replace(/import\("[^"]+"\)\./g, "");
  const text = type => unqualified(checker.typeToString(type, at, ts.TypeFormatFlags.NoTruncation));

  // Option and result types reachable from a signature, by declared name.
  // An inline object type is named after where it appears: `lx.ui.toast(options)`.
  function collectType(type, label, seen = new Set()) {
    if (!type || seen.has(type)) return;
    seen.add(type);
    if (type.isUnion() || type.isIntersection()) return type.types.forEach(t => collectType(t, label, seen));
    const target = type.getSymbol?.();
    if (["Promise", "Array", "ReadonlyArray", "AsyncIterable", "AsyncGenerator"].includes(target?.escapedName)) {
      for (const argument of checker.getTypeArguments(type) ?? []) collectType(argument, label, seen);
      return;
    }
    const declared = type.aliasSymbol?.escapedName ?? target?.escapedName;
    const name = declared && declared !== "__type" && declared !== "__object" ? declared : label;
    if (!(type.flags & ts.TypeFlags.Object) || !ownDeclaration(target) || !name) return;
    if (type.getCallSignatures().length > 0) return;
    if (types.has(name)) return;
    const fields = checker.getPropertiesOfType(type).filter(ownDeclaration);
    // An API object (methods only) is walked as members, not as data.
    if (fields.length && fields.every(field => checker.getTypeOfSymbolAtLocation(field, at).getCallSignatures().length > 0)) return;
    types.set(name, fields.map(field => ({
      name: field.name, summary: summary(field), doc: docText(field), tags: tags(field), optional: !!(field.flags & ts.SymbolFlags.Optional),
      type: text(checker.getTypeOfSymbolAtLocation(field, at)),
    })));
    for (const field of fields) collectType(checker.getTypeOfSymbolAtLocation(field, at), `${name}.${field.name}`, seen);
  }

  function walk(symbol, type, path, depth) {
    for (const property of checker.getPropertiesOfType(type)) {
      // Automation is its own API, documented with `@lingxia/test`.
      if (!ownDeclaration(property) || (depth === 0 && property.name === "automation")) continue;
      const propertyType = checker.getTypeOfSymbolAtLocation(property, at);
      const memberPath = `${path}.${property.name}`;
      const signatures = propertyType.getCallSignatures();
      const optional = !!(property.flags & ts.SymbolFlags.Optional);
      if (signatures.length > 0) {
        members.push({
          path: memberPath, kind: "method", optional, summary: summary(property), doc: docText(property), tags: tags(property),
          signatures: signatures.map(signature => ({
            text: unqualified(checker.signatureToString(signature, at, ts.TypeFormatFlags.NoTruncation)),
            parameters: signature.getParameters().map(parameter => ({
              name: parameter.name,
              type: text(checker.getTypeOfSymbolAtLocation(parameter, at)),
              optional: parameter.valueDeclaration && ts.isParameter(parameter.valueDeclaration) && !!parameter.valueDeclaration.questionToken,
            })),
            returns: text(signature.getReturnType()),
          })),
        });
        for (const signature of signatures) {
          for (const parameter of signature.getParameters()) collectType(checker.getTypeOfSymbolAtLocation(parameter, at), `${memberPath}(${parameter.name})`);
          collectType(signature.getReturnType(), `${memberPath}()`);
        }
        // A callable namespace (`lx.storage()`) also has members.
        if (checker.getPropertiesOfType(propertyType).some(ownDeclaration) && depth < 4) walk(property, propertyType, memberPath, depth + 1);
      } else if (propertyType.flags & ts.TypeFlags.Object && depth < 4) {
        members.push({ path: memberPath, kind: "namespace", optional, summary: summary(property), doc: docText(property), tags: tags(property), signatures: [] });
        walk(property, propertyType, memberPath, depth + 1);
      } else {
        members.push({ path: memberPath, kind: "value", optional, summary: summary(property), doc: docText(property), tags: tags(property), signatures: [],
          type: text(propertyType) });
      }
    }
  }
  walk(lx, checker.getTypeOfSymbolAtLocation(lx, at), "lx", 0);
  return { members, types };
}
