#!/usr/bin/env node

import process from "node:process";

const metadata = JSON.parse(await readStdin());
const releaseNames = process.argv.slice(2);
const releaseSet = new Set(releaseNames);
const workspaceIds = new Set(metadata.workspace_members);
const workspacePackages = metadata.packages.filter((pkg) => workspaceIds.has(pkg.id));
const byName = new Map(workspacePackages.map((pkg) => [pkg.name, pkg]));

const duplicateNames = releaseNames.filter(
  (name, index) => releaseNames.indexOf(name) !== index,
);
const unknownNames = [...releaseSet].filter((name) => !byName.has(name));
const unpublishedNames = [...releaseSet].filter((name) => {
  const pkg = byName.get(name);
  return pkg && !isCratesIoPublishable(pkg);
});
const missingNames = workspacePackages
  .filter(isCratesIoPublishable)
  .map((pkg) => pkg.name)
  .filter((name) => !releaseSet.has(name))
  .sort();

const failures = [
  ["Duplicate crate(s) in the release inventory", [...new Set(duplicateNames)].sort()],
  ["Release inventory contains non-workspace crate(s)", unknownNames.sort()],
  ["Release inventory contains crate(s) excluded from crates.io", unpublishedNames.sort()],
  ["Publishable workspace crate(s) missing from the release inventory", missingNames],
];

let failed = false;
for (const [message, names] of failures) {
  if (names.length === 0) continue;
  console.error(`${message}: ${names.join(", ")}`);
  failed = true;
}
if (failed) process.exit(1);

function isCratesIoPublishable(pkg) {
  return pkg.publish === null || pkg.publish?.includes("crates-io");
}

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  return Buffer.concat(chunks).toString("utf8");
}
