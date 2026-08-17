import { expect, test } from '@rongjs/test';
import type { LxAppDriver } from 'lingxia-types/automation';
import {
  LX_REQUIRED_RUNTIME_SHAPE_NAMES,
  LX_RETURNED_OBJECT_CAPABILITY_NAMES,
  LX_RETURNED_OBJECT_SHAPE_NAMES,
  LX_RETURNED_OBJECT_SURFACES,
  LX_RUNTIME_CAPABILITY_NAMES,
  LX_RUNTIME_SHAPE_NAMES,
} from '../api/manifest.js';
import { showcaseApp } from '../helpers/app.js';
import {
  capabilityLedgerIssues,
  LX_CAPABILITY_LEDGER,
  type CoverageTarget,
} from './capability-ledger.js';
import logicApiCoverage from '../logic-api-coverage.mjs';

export type ContractLayer = 'automation' | 'logic' | 'view' | 'host' | 'native';
export type CoverageLevel = 'shape' | 'semantic' | 'failure' | 'boundary' | 'lifecycle';
export type ContractScope = 'portable' | 'desktop' | 'windows' | 'macos' | 'android' | 'target';
export type CapabilityOutcome =
  | 'supported'
  | 'absent'
  | 'reject'
  | 'no-op'
  | 'external-ui'
  | 'supported-or-absent'
  | 'mixed';

export interface ContractMeta {
  /** Stable identifier used by reports and triage. */
  id: string;
  title: string;
  /** Public capabilities exercised by this case. */
  covers: readonly string[];
  layer: ContractLayer;
  levels: readonly CoverageLevel[];
  scope: ContractScope;
  expectedOutcome: CapabilityOutcome;
}

export interface ContractContext {
  app: LxAppDriver;
  /** Unique prefix for storage keys, files, surfaces, and other mutable fixtures. */
  namespace: string;
  defer(cleanup: () => void | Promise<void>): void;
}

export interface ContractOptions {
  /**
   * Register the case as skipped. The metadata is still declared, so coverage
   * ownership stays intact for the audit, but nothing runs.
   */
  skip?: boolean;
}

export interface EventuallyOptions<T> {
  timeoutMs?: number;
  intervalMs?: number;
  describe: string;
  retryIf?: (error: unknown) => boolean;
  render?: (value: T) => string;
}

const contracts: ContractMeta[] = [];
let sequence = 0;

const PUBLIC_CAPABILITIES = new Set<string>([
  ...LX_RUNTIME_CAPABILITY_NAMES,
  ...LX_RETURNED_OBJECT_CAPABILITY_NAMES,
]);
const PUBLIC_SHAPES = new Set<string>([
  ...LX_RUNTIME_SHAPE_NAMES,
  ...LX_RETURNED_OBJECT_SHAPE_NAMES,
]);

interface CapabilityCoverage {
  capability: string;
  shape: string[];
  behavioral: Array<{
    caseId: string;
    levels: readonly CoverageLevel[];
    outcome: CapabilityOutcome;
    scope: ContractScope;
  }>;
  coveredLevels: CoverageLevel[];
}

function utf8Base64(value: string): string {
  const bytes = new TextEncoder().encode(value);
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function errorText(error: unknown): string {
  return error instanceof Error ? `${error.name}: ${error.message}` : String(error);
}

async function attachFailureScreenshots(app: LxAppDriver, id: string): Promise<void> {
  if (!test.attach) return;
  const safeId = id.replace(/[^a-zA-Z0-9._-]+/g, '-');

  try {
    const page = await app.page.screenshot();
    await test.attach(`${safeId}-page.png`, { mimeType: 'image/png', base64: page.base64 });
  } catch {
    // The original contract failure is more useful than a diagnostic failure.
  }

  try {
    const window = await lx.automation().lxapps.screenshot();
    await test.attach(`${safeId}-window.png`, { mimeType: 'image/png', base64: window.base64 });
  } catch {
    // Some platforms and early-start failures do not have a capturable window.
  }
}

export function contract(
  meta: ContractMeta,
  run: (context: ContractContext) => void | Promise<void>,
  options: ContractOptions = {},
): void {
  contracts.push(meta);
  // Skipping at registration keeps the case out of the pass count. A body that
  // returns early instead would report a green case that asserted nothing.
  const register = options.skip ? test.skip : test;
  register(`${meta.id} | ${meta.title}`, async () => {
    const cleanup: Array<() => void | Promise<void>> = [];
    const app = showcaseApp();
    const namespace = `${meta.id.replace(/[^a-zA-Z0-9]+/g, '-')}-${Date.now()}-${sequence++}`;
    let failed = false;
    let failure: unknown;

    try {
      await run({ app, namespace, defer: (task) => cleanup.push(task) });
    } catch (error) {
      failed = true;
      failure = error;
      await attachFailureScreenshots(app, meta.id);
    }

    const cleanupErrors: unknown[] = [];
    for (let index = cleanup.length - 1; index >= 0; index -= 1) {
      try {
        await cleanup[index]();
      } catch (error) {
        cleanupErrors.push(error);
      }
    }

    if (failed) {
      if (cleanupErrors.length === 0) throw failure;
      const cleanupText = `Cleanup failures:\n${cleanupErrors.map(errorText).join('\n')}`;
      if (failure instanceof Error) {
        failure.message = `${failure.message}\n${cleanupText}`;
        failure.stack = failure.stack ? `${failure.stack}\n${cleanupText}` : failure.message;
        throw failure;
      }
      throw new Error(`${errorText(failure)}\n${cleanupText}`);
    }
    if (cleanupErrors.length > 0) {
      throw new Error(`Contract cleanup failed:\n${cleanupErrors.map(errorText).join('\n')}`);
    }
  });
}

export async function eventually<T>(
  read: () => T | Promise<T>,
  accept: (value: T) => boolean,
  options: EventuallyOptions<T>,
): Promise<T> {
  const timeoutMs = options.timeoutMs ?? 10_000;
  const intervalMs = options.intervalMs ?? 50;
  const deadline = Date.now() + timeoutMs;
  let lastValue: T | undefined;
  let lastError: unknown;

  while (Date.now() < deadline) {
    try {
      lastValue = await read();
      lastError = undefined;
      if (accept(lastValue)) return lastValue;
    } catch (error) {
      if (!options.retryIf?.(error)) throw error;
      lastError = error;
    }
    await new Promise<void>((resolve) => setTimeout(() => resolve(), intervalMs));
  }

  const observed = lastError === undefined
    ? options.render?.(lastValue as T) ?? JSON.stringify(lastValue)
    : errorText(lastError);
  throw new Error(`Timed out waiting for ${options.describe}; last observed: ${observed}`);
}

export async function expectReject(
  operation: () => Promise<unknown>,
  expected: { code?: string; message?: string | RegExp },
): Promise<void> {
  let received: unknown;
  try {
    await operation();
  } catch (error) {
    received = error;
  }

  expect(received).toBeDefined();
  const record = received as { code?: unknown; message?: unknown };
  if (expected.code !== undefined) expect(record.code).toBe(expected.code);
  if (typeof expected.message === 'string') expect(String(record.message)).toContain(expected.message);
  if (expected.message instanceof RegExp) expect(String(record.message)).toMatch(expected.message);
}

export function registerContractAudit(options: { requireCanonicalShape?: boolean } = {}): void {
  test('contract metadata is complete and unique', async () => {
    const ids = contracts.map(({ id }) => id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(contracts.every(({ expectedOutcome, id, covers, levels, scope, title }) => (
      id.length > 0
      && title.length > 0
      && covers.length > 0
      && levels.length > 0
      && scope.length > 0
      && expectedOutcome.length > 0
    )))
      .toBeTruthy();

    const knownCoverage = new Set([...PUBLIC_CAPABILITIES, ...PUBLIC_SHAPES]);
    const unknownCoverage = contracts.flatMap(({ covers, id }) => (
      covers
        .filter((capability) => !knownCoverage.has(capability))
        .map((capability) => ({ capability, id }))
    ));
    expect(unknownCoverage).toEqual([]);

    expect(capabilityLedgerIssues()).toEqual({ duplicates: [], missing: [], unknown: [] });

    const args = test.args as Record<string, string>;
    const target = ['windows', 'macos', 'android'].includes(args.platform)
      ? args.platform as CoverageTarget
      : undefined;
    const missingLogicOwners = logicApiCoverage.apis.flatMap((requirement) => {
      if (requirement.mode !== 'automated') return [];
      if ('targets' in requirement
        && requirement.targets
        && (!target || !requirement.targets.includes(target))) return [];
      const owner = contracts.find(({ id }) => id === requirement.owner);
      return owner?.covers.includes(requirement.api)
        ? []
        : [{ api: requirement.api, owner: requirement.owner, target: target ?? null }];
    });
    expect(missingLogicOwners).toEqual([]);
    const missingRequiredCoverage = LX_CAPABILITY_LEDGER.flatMap((requirement) => {
      if (requirement.mode !== 'automated' || !requirement.ownerCaseId) return [];
      if (requirement.requiredTargets && (!target || !requirement.requiredTargets.includes(target))) return [];
      const owner = contracts.find(({ id }) => id === requirement.ownerCaseId);
      if (!owner || !owner.covers.includes(requirement.capability)) {
        return [{
          capability: requirement.capability,
          missing: requirement.requiredLevels ?? [],
          ownerCaseId: requirement.ownerCaseId,
        }];
      }
      const missing = (requirement.requiredLevels ?? []).filter((level) => !owner.levels.includes(level));
      return missing.length > 0
        ? [{ capability: requirement.capability, missing, ownerCaseId: requirement.ownerCaseId }]
        : [];
    });
    expect(missingRequiredCoverage).toEqual([]);

    if (options.requireCanonicalShape) {
      const covered = new Set(contracts.flatMap(({ covers }) => covers));
      expect(LX_REQUIRED_RUNTIME_SHAPE_NAMES.filter((capability) => !covered.has(capability))).toEqual([]);
    }

    if (test.attach) {
      const coverage: CapabilityCoverage[] = LX_RUNTIME_CAPABILITY_NAMES.map((capability) => {
        const shapeName = `shape:${capability}`;
        const shape = contracts
          .filter(({ covers }) => covers.includes(shapeName))
          .map(({ id }) => id);
        const behavioral = contracts
          .filter(({ covers, levels }) => covers.includes(capability) && levels.some((level) => level !== 'shape'))
          .map(({ expectedOutcome, id, levels, scope }) => ({
            caseId: id,
            levels,
            outcome: expectedOutcome,
            scope,
          }));
        const coveredLevels = Array.from(new Set([
          ...(shape.length > 0 ? ['shape' as const] : []),
          ...behavioral.flatMap(({ levels }) => levels),
        ]));
        return { behavioral, capability, coveredLevels, shape };
      });
      await test.attach('contract-coverage.json', {
        mimeType: 'application/json',
        base64: utf8Base64(JSON.stringify({
          target: {
            framework: args.framework ?? null,
            platform: args.platform ?? null,
          },
          summary: {
            capabilities: coverage.length,
            capabilitiesWithBehavior: coverage.filter(({ behavioral }) => behavioral.length > 0).length,
            contracts: contracts.length,
          },
          logicUsage: {
            summary: Object.fromEntries(
              ['automated', 'external-fixture', 'external-ui', 'optional-provider', 'destructive']
                .map((mode) => [
                  mode,
                  logicApiCoverage.apis.filter((entry) => entry.mode === mode).length,
                ]),
            ),
            apis: logicApiCoverage.apis,
          },
          ledger: LX_CAPABILITY_LEDGER,
          returnedObjects: LX_RETURNED_OBJECT_SURFACES.map((surface) => ({
            factory: surface.factory,
            fixture: surface.fixture,
            name: surface.name,
            optionalProperties: surface.optionalProperties,
            runtimeShapeCaseIds: contracts
              .filter(({ covers }) => surface.members.some((member) => covers.includes(`shape:${surface.name}.${member}`)))
              .map(({ id }) => id),
          })),
          coverage,
          contracts,
        }, null, 2)),
      });
    }
  });
}
