// Every semantic role in the View schema has its own case in each native role table.
// A host meeting an unmapped role reads it as a plain group, so a missing case is caught here.
// Harmony maps the Rust `Role` enum with an exhaustive match and needs no check.
//   node scripts/ci/check-semantic-roles.mjs
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = join(dirname(fileURLToPath(import.meta.url)), '../..');
const read = (path) => readFileSync(join(repo, path), 'utf8');
const roles = JSON.parse(read('contracts/view/schema.json')).semantics.roles;

// The body of the role switch: from the function's signature to its `default:`.
function switchBody(path, signature) {
  const text = read(path);
  const start = text.indexOf(signature);
  const end = start < 0 ? -1 : text.indexOf('default:', start);
  if (end < 0) throw new Error(`${path}: no \`${signature}\` switch with a default`);
  return text.slice(start, end);
}

const constant = (role) => `GpuSemantics.ROLE_${role.toUpperCase().replaceAll('-', '_')}`;
const tables = [
  {
    path: 'lingxia-sdk/apple/Sources/Platform/macOS/View/MacGpuAccessibility.swift',
    signature: 'static func role(',
    has: (body, role) => body.includes(`"${role}"`),
  },
  {
    path: 'lingxia-sdk/apple/Sources/Platform/iOS/View/GpuAccessibility.swift',
    signature: 'static func traits(',
    has: (body, role) => body.includes(`"${role}"`),
  },
  {
    path: 'lingxia-sdk/android/lingxia/src/main/java/com/lingxia/view/GpuAccessibility.java',
    signature: 'static String className(',
    has: (body, role) => new RegExp(`case ${constant(role)}:`).test(body),
  },
];

const problems = [];
for (const { path, signature, has } of tables) {
  const body = switchBody(path, signature);
  for (const role of roles) if (!has(body, role)) problems.push(`${path}: no case for role "${role}"`);
}
if (problems.length) {
  console.error(`Semantic roles in contracts/view/schema.json without a native mapping:\n  ${problems.join('\n  ')}`);
  process.exit(1);
}
console.log(`${roles.length} semantic roles mapped on macOS, iOS and Android`);
