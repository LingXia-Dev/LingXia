import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import ts from 'typescript';
import {
  LX_RETURNED_OBJECT_SURFACES,
  LX_RUNTIME_SURFACES,
} from 'lingxia-types/testing';
import manifest from '../logic-api-coverage.mjs';

const root = path.resolve(import.meta.dirname, '../..');

function logicFiles() {
  const pages = path.join(root, 'pages');
  return [
    path.join(root, 'lxapp.ts'),
    ...fs.readdirSync(pages, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => path.join(pages, entry.name, 'index.ts'))
      .filter((file) => fs.existsSync(file)),
  ];
}

function directLxPath(node) {
  const members = [];
  let current = node;
  while (ts.isPropertyAccessExpression(current)) {
    members.unshift(current.name.text);
    current = current.expression;
  }
  return ts.isIdentifier(current) && current.text === 'lx'
    ? `lx.${members.join('.')}`
    : null;
}

function scan() {
  const usages = new Map();
  for (const file of logicFiles()) {
    const source = fs.readFileSync(file, 'utf8');
    const ast = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true);
    const visit = (node) => {
      if (ts.isPropertyAccessExpression(node)) {
        const isNestedExpression = ts.isPropertyAccessExpression(node.parent)
          && node.parent.expression === node;
        if (!isNestedExpression) {
          const api = directLxPath(node);
          if (api) {
            const location = ast.getLineAndCharacterOfPosition(node.getStart(ast));
            const relative = path.relative(root, file).replaceAll('\\', '/');
            const refs = usages.get(api) ?? [];
            refs.push(`${relative}:${location.line + 1}`);
            usages.set(api, refs);
          }
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(ast);
  }
  return usages;
}

function unsupportedLxSyntax() {
  const issues = [];
  for (const file of logicFiles()) {
    const source = fs.readFileSync(file, 'utf8');
    const ast = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true);
    const visit = (node) => {
      if (ts.isIdentifier(node)
        && node.text === 'lx'
        && !(ts.isPropertyAccessExpression(node.parent) && node.parent.expression === node)
        && !(ts.isQualifiedName(node.parent) && node.parent.left === node)) {
        const location = ast.getLineAndCharacterOfPosition(node.getStart(ast));
        issues.push(`${path.relative(root, file).replaceAll('\\', '/')}:${location.line + 1}`);
      }
      ts.forEachChild(node, visit);
    };
    visit(ast);
  }
  return issues;
}

const objectSurfaces = [
  ...LX_RETURNED_OBJECT_SURFACES,
  ...LX_RUNTIME_SURFACES
    .filter(({ expression, layer, name }) => layer === 'logic' && name !== 'lx' && expression.endsWith('()'))
    .map((surface) => ({ ...surface, factory: surface.expression.slice(0, -2) })),
];
const surfacesByFactory = new Map();
for (const surface of objectSurfaces) {
  const entries = surfacesByFactory.get(surface.factory) ?? [];
  entries.push(surface);
  surfacesByFactory.set(surface.factory, entries);
}

function unwrapExpression(node) {
  let current = node;
  while (ts.isAwaitExpression(current)
    || ts.isParenthesizedExpression(current)
    || ts.isAsExpression(current)
    || ts.isNonNullExpression(current)) {
    current = current.expression;
  }
  return current;
}

function receiverKey(node, ast) {
  const current = unwrapExpression(node);
  if (ts.isIdentifier(current)) return current.text;
  if (ts.isPropertyAccessExpression(current)
    && (ts.isThis(current.expression) || ts.isIdentifier(current.expression))) {
    return current.getText(ast);
  }
  return null;
}

function scanReturnedObjects() {
  const usages = new Map();
  for (const file of logicFiles()) {
    const source = fs.readFileSync(file, 'utf8');
    const ast = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true);
    const aliases = new Map();
    const methodReturns = new Map();

    // The content source is the function now, so the factory path alone says
    // which handle comes back — no spec-shape guessing.
    const factorySurfaces = (expression) => {
      const current = unwrapExpression(expression);
      if (!ts.isCallExpression(current)) return null;
      const factory = directLxPath(current.expression);
      return factory ? surfacesByFactory.get(factory) ?? null : null;
    };
    const inferred = (expression) => {
      const direct = factorySurfaces(expression);
      if (direct) return direct;
      const current = unwrapExpression(expression);
      const key = receiverKey(current, ast);
      if (key && aliases.has(key)) return aliases.get(key);
      if (ts.isCallExpression(current)) {
        const method = receiverKey(current.expression, ast);
        if (method && methodReturns.has(method)) return methodReturns.get(method);
      }
      return null;
    };
    const assign = (target, value) => {
      const key = receiverKey(target, ast);
      const surfaces = inferred(value);
      if (key && surfaces) aliases.set(key, surfaces);
    };

    for (let pass = 0; pass < 3; pass += 1) {
      const discover = (node) => {
        if (ts.isVariableDeclaration(node) && node.initializer) assign(node.name, node.initializer);
        if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.EqualsToken) {
          assign(node.left, node.right);
        }
        if ((ts.isMethodDeclaration(node) || ts.isPropertyAssignment(node)) && node.name) {
          const methodName = node.name.getText(ast).replaceAll(/["']/g, '');
          const body = ts.isMethodDeclaration(node)
            ? node.body
            : (ts.isFunctionExpression(node.initializer) || ts.isArrowFunction(node.initializer))
              ? node.initializer.body
              : null;
          if (body) {
            const inspectReturn = (candidate) => {
              if (ts.isReturnStatement(candidate) && candidate.expression) {
                const surfaces = inferred(candidate.expression);
                if (surfaces) methodReturns.set(`this.${methodName}`, surfaces);
              }
              ts.forEachChild(candidate, inspectReturn);
            };
            inspectReturn(body);
          }
        }
        if (ts.isCallExpression(node)) {
          const direct = directLxPath(node.expression);
          if (direct === 'lx.surface.onContext') {
            const callback = node.arguments[0];
            if ((ts.isArrowFunction(callback) || ts.isFunctionExpression(callback)) && callback.parameters[0]) {
              aliases.set(callback.parameters[0].name.getText(ast), [
                objectSurfaces.find(({ name }) => name === 'PageSurface'),
              ].filter(Boolean));
            }
          }
          const receiver = ts.isPropertyAccessExpression(node.expression)
            ? inferred(node.expression.expression)
            : null;
          if (receiver?.some(({ name }) => name === 'UpdateManager')
            && ts.isPropertyAccessExpression(node.expression)
            && node.expression.name.text === 'onUpdateReady') {
            const callback = node.arguments[0];
            if ((ts.isArrowFunction(callback) || ts.isFunctionExpression(callback)) && callback.parameters[0]) {
              aliases.set(callback.parameters[0].name.getText(ast), [
                objectSurfaces.find(({ name }) => name === 'HostAppUpdateInfo'),
              ].filter(Boolean));
            }
          }
        }
        ts.forEachChild(node, discover);
      };
      discover(ast);
    }

    const record = (node) => {
      if (ts.isPropertyAccessExpression(node)) {
        const surfaces = inferred(node.expression);
        if (surfaces) {
          for (const surface of surfaces) {
            if (!surface.members.includes(node.name.text)) continue;
            const api = `${surface.name}.${node.name.text}`;
            const location = ast.getLineAndCharacterOfPosition(node.getStart(ast));
            const relative = path.relative(root, file).replaceAll('\\', '/');
            const refs = usages.get(api) ?? [];
            refs.push(`${relative}:${location.line + 1}`);
            usages.set(api, refs);
          }
        }
      }
      ts.forEachChild(node, record);
    };
    record(ast);
  }
  return usages;
}

function filesBelow(directory, suffix) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const candidate = path.join(directory, entry.name);
    if (entry.isDirectory()) return filesBelow(candidate, suffix);
    return entry.name.endsWith(suffix) ? [candidate] : [];
  });
}

function specCoverage() {
  const result = new Map();
  for (const file of filesBelow(path.join(root, 'tests'), '.test.ts')) {
    const source = fs.readFileSync(file, 'utf8');
    const ast = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true);
    const visit = (node) => {
      if (ts.isCallExpression(node) && node.arguments.length >= 2) {
        const options = node.arguments.find((argument) => ts.isObjectLiteralExpression(argument));
        if (options && ts.isObjectLiteralExpression(options)) {
          const properties = new Map(options.properties
            .filter(ts.isPropertyAssignment)
            .map((property) => [property.name.getText(ast).replaceAll(/["']/g, ''), property.initializer]));
          const idNode = properties.get('id');
          const coversNode = properties.get('covers');
          if (idNode && ts.isStringLiteral(idNode) && coversNode && ts.isArrayLiteralExpression(coversNode)) {
            result.set(idNode.text, new Set(coversNode.elements.filter(ts.isStringLiteral).map((item) => item.text)));
          }
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(ast);
  }
  return result;
}

const scanned = scan();
for (const [api, refs] of scanReturnedObjects()) scanned.set(api, refs);
if (process.argv.includes('--print')) {
  process.stdout.write(`${JSON.stringify(Object.fromEntries(scanned), null, 2)}\n`);
  process.exit(0);
}

const declared = new Map(manifest.apis.map((entry) => [entry.api, entry]));
const specs = specCoverage();
const missing = [...scanned.keys()].filter((api) => !declared.has(api));
const stale = [...declared.keys()].filter((api) => !scanned.has(api));
const invalid = manifest.apis.filter((entry) => (
  !entry.api
  || !['automated', 'external-fixture', 'external-ui', 'optional-provider', 'destructive'].includes(entry.mode)
  || !entry.owner
));
const duplicates = manifest.apis
  .map(({ api }) => api)
  .filter((api, index, all) => all.indexOf(api) !== index);
const ownerErrors = manifest.apis
  .filter(({ mode }) => mode === 'automated')
  .flatMap(({ api, owner }) => {
    const covers = specs.get(owner);
    if (!covers) return [{ api, owner, reason: 'spec id not found' }];
    return covers.has(api) ? [] : [{ api, owner, reason: 'spec does not cover API' }];
  });
const unsupportedSyntax = unsupportedLxSyntax();

if (missing.length || stale.length || invalid.length || duplicates.length || ownerErrors.length || unsupportedSyntax.length) {
  process.stderr.write(`${JSON.stringify({ duplicates, invalid, missing, ownerErrors, stale, unsupportedSyntax }, null, 2)}\n`);
  process.exit(1);
}

const modes = Object.fromEntries(
  ['automated', 'external-fixture', 'external-ui', 'optional-provider', 'destructive'].map((mode) => [
    mode,
    manifest.apis.filter((entry) => entry.mode === mode).length,
  ]),
);
process.stdout.write(`Showcase Logic lx API inventory: ${scanned.size}/${declared.size} mapped ${JSON.stringify(modes)}\n`);
