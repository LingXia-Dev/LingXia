// The Logic API as a list of members, read from the declarations the way an editor sees them:
// every path under `lx`, its signature and summary, and the option and result types it reaches.
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const packageDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");

// `entries: true` adds the rest of the authored surface: `App` and its hooks, the page API
// (`definePage`, `page.*`) and the error helpers, with each type those modules export.
export function catalogue({ entries = false } = {}) {
  const entry = resolve(packageDir, "src/index.ts");
  const globals = resolve(packageDir, "typegen/catalogue-lx.d.ts");
  const roots = entries ? [entry, globals, resolve(packageDir, "src/page.ts")] : [entry, globals];
  const program = ts.createProgram(roots, {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.NodeNext, moduleResolution: ts.ModuleResolutionKind.NodeNext,
    strict: true, noEmit: true, skipLibCheck: true, types: [],
  });
  const checker = program.getTypeChecker();
  const lx = checker.resolveName("lx", undefined, ts.SymbolFlags.Value, false);
  if (!lx) throw Error("no `lx` in typegen/catalogue-lx.d.ts");
  const at = lx.valueDeclaration;
  const members = [];
  const types = new Map();
  // Per-type docs and anchors, and named aliases that are not object types (unions, literals).
  const typeInfo = new Map();
  const aliases = new Map();
  const ownDeclaration = symbol => symbol?.declarations?.some(d => !d.getSourceFile().fileName.includes("/typescript/lib/"));
  // The first sentence of the first paragraph.
  const summary = symbol => {
    const paragraph = ts.displayPartsToString(symbol.getDocumentationComment(checker)).split(/\n\s*\n/)[0].replace(/\s+/g, " ").trim();
    return paragraph.match(/^.*?[.!?](?=\s|$)/)?.[0] ?? paragraph;
  };
  const docText = symbol => ts.displayPartsToString(symbol.getDocumentationComment(checker));
  const tags = symbol => symbol.getJsDocTags(checker).map(tag => tag.name);
  const jsdoc = symbol => symbol.getJsDocTags(checker).map(tag => ({ name: tag.name, text: ts.displayPartsToString(tag.text ?? []) }));
  const source = symbol => {
    const declaration = symbol?.declarations?.find(d => !d.getSourceFile().fileName.includes("/node_modules/"));
    if (!declaration) return null;
    const file = declaration.getSourceFile();
    return { file: relative(packageDir, file.fileName).split(sep).join("/"), line: file.getLineAndCharacterOfPosition(declaration.getStart()).line + 1 };
  };
  const describe = symbol => ({ summary: summary(symbol), doc: docText(symbol), jsdoc: jsdoc(symbol), source: source(symbol) });
  const unqualified = value => value.replace(/import\("[^"]+"\)\./g, "");
  const text = type => unqualified(checker.typeToString(type, at, ts.TypeFormatFlags.NoTruncation));

  // Option and result types reachable from a signature, by declared name.
  // An inline object type is named after where it appears: `lx.ui.toast(options)`.
  // A union of literals spelled out (`LxErrorCode` is `1000 | 1001 | …`); anything else as written.
  const printer = ts.createPrinter({ removeComments: true });
  function aliasText(symbol, type) {
    if (type.isUnion() && type.types.every(t => t.isLiteral() || t.flags & ts.TypeFlags.BooleanLiteral)) {
      return unqualified(checker.typeToString(type, at, ts.TypeFormatFlags.NoTruncation | ts.TypeFormatFlags.InTypeAlias));
    }
    const declaration = symbol.declarations.find(ts.isTypeAliasDeclaration);
    if (!declaration) return text(type);
    return printer.printNode(ts.EmitHint.Unspecified, declaration.type, declaration.getSourceFile()).replace(/\s+/g, " ");
  }
  // A parameter's type as declared, so an overload reads `AppDownloadOptions & { to: string }`.
  function writtenType(parameter) {
    const node = parameter.valueDeclaration?.type;
    return node ? printer.printNode(ts.EmitHint.Unspecified, node, node.getSourceFile()).replace(/\s+/g, " ") : null;
  }
  function writtenSignature(signature) {
    const node = signature.declaration;
    if (!node?.parameters || !node.type) return null;
    const print = part => printer.printNode(ts.EmitHint.Unspecified, part, node.getSourceFile()).replace(/\s+/g, " ");
    const typeParameters = node.typeParameters?.length ? `<${node.typeParameters.map(print).join(", ")}>` : "";
    return `${typeParameters}(${node.parameters.map(print).join(", ")}): ${print(node.type)}`;
  }
  const fieldEntry = field => ({
    name: field.name, summary: summary(field), doc: docText(field), tags: tags(field), optional: !!(field.flags & ts.SymbolFlags.Optional),
    type: text(checker.getTypeOfSymbolAtLocation(field, at)), jsdoc: jsdoc(field), source: source(field),
  });
  function collectType(type, label, seen = new Set(), force = false) {
    if (!type || seen.has(type)) return;
    seen.add(type);
    const alias = type.aliasSymbol;
    if (alias && ownDeclaration(alias) && !(type.flags & (ts.TypeFlags.Object | ts.TypeFlags.Intersection)) && !aliases.has(alias.escapedName)) {
      aliases.set(alias.escapedName, { ...describe(alias), text: aliasText(alias, type) });
    }
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
    if (!force && fields.length && fields.every(field => checker.getTypeOfSymbolAtLocation(field, at).getCallSignatures().length > 0)) return;
    types.set(name, fields.map(fieldEntry));
    const named = type.aliasSymbol ?? target;
    typeInfo.set(name, named && ownDeclaration(named) && name === declared ? describe(named) : { summary: "", doc: "", jsdoc: [], source: source(target) });
    for (const field of fields) collectType(checker.getTypeOfSymbolAtLocation(field, at), `${name}.${field.name}`, seen);
  }

  function walk(symbol, type, path, depth, roots) {
    for (const property of roots ?? checker.getPropertiesOfType(type)) {
      // Automation is its own API, documented with `@lingxia/test`.
      if (!ownDeclaration(property) || (depth === 0 && property.name === "automation")) continue;
      // Outside `lx`, an optional hook or field (`App.onShow`, `page.opener`) is read as what it holds.
      const declaredType = checker.getTypeOfSymbolAtLocation(property, at);
      const propertyType = path.startsWith("lx") ? declaredType : checker.getNonNullableType(declaredType);
      const memberPath = path ? `${path}.${property.name}` : property.name;
      const signatures = propertyType.getCallSignatures();
      const optional = !!(property.flags & ts.SymbolFlags.Optional);
      if (signatures.length > 0) {
        members.push({
          path: memberPath, kind: "method", optional, summary: summary(property), doc: docText(property), tags: tags(property), jsdoc: jsdoc(property), source: source(property),
          signatures: signatures.map(signature => ({
            text: unqualified(checker.signatureToString(signature, at, ts.TypeFormatFlags.NoTruncation)),
            written: writtenSignature(signature),
            parameters: signature.getParameters().map(parameter => ({
              name: parameter.name,
              type: text(checker.getTypeOfSymbolAtLocation(parameter, at)),
              written: writtenType(parameter),
              optional: !!(parameter.valueDeclaration && ts.isParameter(parameter.valueDeclaration) && (parameter.valueDeclaration.questionToken || parameter.valueDeclaration.initializer)),
            })),
            returns: text(signature.getReturnType()),
          })),
        });
        for (const signature of signatures) {
          for (const parameter of signature.getParameters()) collectType(checker.getTypeOfSymbolAtLocation(parameter, at), `${memberPath}(${parameter.name})`);
          collectType(signature.getReturnType(), `${memberPath}()`);
        }
        // A callable namespace also has members.
        if (checker.getPropertiesOfType(propertyType).some(ownDeclaration) && depth < 4) walk(property, propertyType, memberPath, depth + 1);
      } else if (propertyType.flags & (ts.TypeFlags.Object | ts.TypeFlags.Intersection) && checker.getPropertiesOfType(propertyType).some(ownDeclaration) && depth < 4) {
        members.push({ path: memberPath, kind: "namespace", optional, summary: summary(property), doc: docText(property), tags: tags(property), jsdoc: jsdoc(property), source: source(property), signatures: [] });
        walk(property, propertyType, memberPath, depth + 1);
      } else {
        members.push({ path: memberPath, kind: "value", optional, summary: summary(property), doc: docText(property), tags: tags(property), jsdoc: jsdoc(property), source: source(property), signatures: [],
          type: text(propertyType) });
        if (entries) collectType(propertyType, memberPath);
      }
    }
  }
  walk(lx, checker.getTypeOfSymbolAtLocation(lx, at), "lx", 0);
  const exportsByEntry = {};
  if (entries) {
    // `lx.automation()` itself, without the test API behind it.
    const automation = checker.getPropertyOfType(checker.getTypeOfSymbolAtLocation(lx, at), "automation");
    if (automation) {
      const [before, known] = [members.length, new Set([...types.keys(), ...aliases.keys()])];
      walk(null, null, "lx", 1, [automation]);
      members.splice(before + 1);
      for (const map of [types, aliases, typeInfo]) for (const name of [...map.keys()]) if (!known.has(name)) map.delete(name);
    }
    for (const [file, entry] of [["src/index.ts", "@lingxia/types"], ["src/page.ts", "@lingxia/types/page"], ["src/error.ts", "@lingxia/types/error"]]) {
      const module = checker.getSymbolAtLocation(program.getSourceFile(resolve(packageDir, file)));
      exportsByEntry[entry] = checker.getExportsOfModule(module).map(s => s.name).sort();
    }
    // `@lingxia/logic` is where Logic imports from: all three, plus the host's `lx`.
    exportsByEntry["@lingxia/logic"] = [...new Set([...exportsByEntry["@lingxia/types"], ...exportsByEntry["@lingxia/types/page"], "lx"])].sort();
    const start = members.length;
    const app = checker.resolveName("App", undefined, ts.SymbolFlags.Value, false);
    const logicModule = checker.getSymbolAtLocation(program.getSourceFile(resolve(packageDir, "src/generated/logic.ts")));
    const appConfig = checker.getExportsOfModule(logicModule).find(s => s.name === "AppConfig");
    walk(null, null, "", 0, [app]);
    if (appConfig) walk(appConfig, checker.getDeclaredTypeOfSymbol(appConfig), "App", 1);
    for (const member of members.slice(start)) member.entry = "global";
    for (const [file, entry] of [["src/page.ts", "@lingxia/logic"], ["src/error.ts", "@lingxia/logic"]]) {
      const before = members.length;
      const module = checker.getSymbolAtLocation(program.getSourceFile(resolve(packageDir, file)));
      const exported = checker.getExportsOfModule(module).map(s => (s.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(s) : s)).filter(ownDeclaration);
      walk(null, null, "", 0, exported.filter(s => s.flags & (ts.SymbolFlags.Function | ts.SymbolFlags.Variable)));
      for (const symbol of exported.filter(s => s.flags & (ts.SymbolFlags.Interface | ts.SymbolFlags.TypeAlias))) {
        if (symbol.name === "PageApi") walk(symbol, checker.getDeclaredTypeOfSymbol(symbol), "page", 1);
        recordType(symbol, true);
      }
      for (const member of members.slice(before)) member.entry = entry;
    }
    // Every other named type, so a union (`DownloadResult`) or a type no member names is found by name.
    for (const file of ["src/generated/logic.ts"]) {
      const module = checker.getSymbolAtLocation(program.getSourceFile(resolve(packageDir, file)));
      for (const symbol of checker.getExportsOfModule(module)) {
        if (symbol.flags & (ts.SymbolFlags.Interface | ts.SymbolFlags.TypeAlias) && ownDeclaration(symbol)) recordType(symbol, false);
      }
    }
  }
  function recordType(symbol, force) {
    const name = symbol.name;
    if (types.has(name) || aliases.has(name)) return;
    const declared = checker.getDeclaredTypeOfSymbol(symbol);
    const declaration = symbol.declarations.find(ts.isTypeAliasDeclaration);
    if (declared.isIntersection()) {
      const fields = checker.getPropertiesOfType(declared).filter(ownDeclaration);
      types.set(name, fields.map(fieldEntry));
      typeInfo.set(name, describe(symbol));
      for (const field of fields) collectType(checker.getTypeOfSymbolAtLocation(field, at), `${name}.${field.name}`);
      return;
    }
    if (!declared.isUnion() && declared.flags & ts.TypeFlags.Object) {
      collectType(declared, name, new Set(), true);
      if (types.has(name)) {
        if (!typeInfo.get(name)?.summary) typeInfo.set(name, describe(symbol));
        return;
      }
    }
    if (!force && !declaration) return;
    aliases.set(name, { ...describe(symbol), text: aliasText(symbol, declared) });
    // Named arms of a union are recorded under their own names.
    if (declared.isUnion()) collectType(declared, undefined);
  }
  return { members, types, typeInfo, aliases, exports: exportsByEntry };
}
