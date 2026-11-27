// Every `lx` function that takes arguments has an argument table under
// tests/api/arguments, or a reason here why it has none.
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import ts from 'typescript';
import { LX_CATALOGUE_KINDS } from '@lingxia/types/testing';

const root = path.resolve(import.meta.dirname, '../..');
const tablesDir = path.join(root, 'tests/api/arguments');

/** Functions that take arguments but have no table, and why. */
const EXEMPT = {
  'lx.host.terminal.conpty.install': 'Windows only, and installs a runtime',
  'lx.host.terminal.conpty.setEnabled': 'Windows only, and changes the shell for every terminal',
};

// The `lx` type as an lxapp sees it, from a file the Logic project would hold.
const configPath = path.join(root, 'tsconfig.logic.json');
const parsed = ts.getParsedCommandLineOfConfigFile(configPath, {}, {
  ...ts.sys,
  onUnRecoverableConfigFileDiagnostic(diagnostic) {
    throw new Error(ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n'));
  },
});
if (!parsed) throw new Error(`Cannot read ${configPath}`);
const probeFile = path.join(root, '__argument_coverage_probe__.ts');
const probeSource = "import { lx } from '@lingxia/logic';\nexport const probe = lx;\n";
const host = ts.createCompilerHost(parsed.options, true);
const readFile = host.readFile.bind(host);
const fileExists = host.fileExists.bind(host);
const getSourceFile = host.getSourceFile.bind(host);
host.readFile = (file) => (path.resolve(file) === probeFile ? probeSource : readFile(file));
host.fileExists = (file) => path.resolve(file) === probeFile || fileExists(file);
host.getSourceFile = (file, languageVersion, ...rest) => (path.resolve(file) === probeFile
  ? ts.createSourceFile(file, probeSource, languageVersion, true)
  : getSourceFile(file, languageVersion, ...rest));
const program = ts.createProgram({
  rootNames: [...parsed.fileNames, probeFile],
  options: parsed.options,
  host,
});
const checker = program.getTypeChecker();
const probe = program.getSourceFile(probeFile);
const declaration = probe.statements
  .filter(ts.isVariableStatement)
  .flatMap((statement) => statement.declarationList.declarations)[0];
const lxType = checker.getTypeAtLocation(declaration.name);

function memberType(type, name) {
  const symbol = checker.getPropertyOfType(checker.getApparentType(type), name);
  return symbol ? checker.getTypeOfSymbolAtLocation(symbol, declaration) : null;
}

/** Whether any call signature of `lx.<path>` declares a parameter. */
function takesArguments(api) {
  let type = lxType;
  for (const key of api.split('.').slice(1)) {
    type = type && memberType(type, key);
  }
  if (!type) throw new Error(`${api} is in the catalogue but not on the lx type`);
  return checker.getSignaturesOfType(type, ts.SignatureKind.Call)
    .some((signature) => signature.getParameters().length > 0);
}

// `argumentContracts({ 'fs.write': … })` keys name the tables.
function tableKeys() {
  const keys = new Map();
  for (const name of fs.readdirSync(tablesDir).filter((file) => file.endsWith('.test.ts'))) {
    const file = path.join(tablesDir, name);
    const ast = ts.createSourceFile(file, fs.readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true);
    const visit = (node) => {
      if (ts.isCallExpression(node)
        && ts.isIdentifier(node.expression)
        && node.expression.text === 'argumentContracts'
        && node.arguments[0]
        && ts.isObjectLiteralExpression(node.arguments[0])) {
        for (const property of node.arguments[0].properties) {
          const key = property.name && (ts.isIdentifier(property.name) || ts.isStringLiteral(property.name))
            ? property.name.text
            : null;
          if (!key) throw new Error(`${name}: a table key must be a literal`);
          if (keys.has(`lx.${key}`)) throw new Error(`lx.${key} has two tables`);
          keys.set(`lx.${key}`, name);
        }
      }
      node.forEachChild(visit);
    };
    visit(ast);
  }
  return keys;
}

const functions = Object.entries(LX_CATALOGUE_KINDS)
  .filter(([, kind]) => kind === 'sync' || kind === 'async')
  .map(([api]) => api)
  .sort();
const withArguments = functions.filter(takesArguments);
const tables = tableKeys();

const missing = withArguments.filter((api) => !tables.has(api) && !EXEMPT[api]);
const unknown = [...tables.keys()].filter((api) => !functions.includes(api));
const noArguments = [...tables.keys()].filter((api) => functions.includes(api) && !withArguments.includes(api));
const staleExempt = Object.keys(EXEMPT).filter((api) => !withArguments.includes(api) || tables.has(api));

if (missing.length || unknown.length || noArguments.length || staleExempt.length) {
  console.error(JSON.stringify({ missing, unknown, noArguments, staleExempt }, null, 2));
  process.exit(1);
}
console.log(`argument coverage ok: ${tables.size} tables for ${withArguments.length} functions that take arguments, ${Object.keys(EXEMPT).length} exempt`);
