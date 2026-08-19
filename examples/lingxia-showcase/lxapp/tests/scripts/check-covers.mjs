import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import {
  LX_REQUIRED_RUNTIME_SHAPE_NAMES,
} from 'lingxia-types/testing';
import manifest from '../logic-api-coverage.mjs';

const reportPath = process.argv[2] ?? process.env.LXDEV_TEST_REPORT;
if (!reportPath) {
  console.error('usage: node tests/scripts/check-covers.mjs <report.json>');
  console.error('Only a full unfiltered passing run refreshes the covers gate.');
  process.exit(2);
}

const report = JSON.parse(fs.readFileSync(path.resolve(reportPath), 'utf8'));
if (report.partial) {
  console.error('refusing to refresh covers from a partial report');
  process.exit(2);
}
if (report.filtered) {
  console.error('refusing to refresh covers from a filtered (--grep / spec.only) run');
  process.exit(2);
}

const passing = (report.cases ?? []).filter((item) => item.status === 'passed');
const covered = new Set(passing.flatMap((item) => item.covers ?? []));

const missingLogic = manifest.apis.flatMap((requirement) => {
  if (requirement.mode !== 'automated') return [];
  return covered.has(requirement.api) ? [] : [requirement.api];
});
const missingShapes = LX_REQUIRED_RUNTIME_SHAPE_NAMES.filter((name) => !covered.has(name));

if (missingLogic.length > 0 || missingShapes.length > 0) {
  console.error(JSON.stringify({ missingLogic, missingShapes }, null, 2));
  process.exit(1);
}

console.log(`covers gate ok: ${covered.size} tags from ${passing.length} passing specs`);
