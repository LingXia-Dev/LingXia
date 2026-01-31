import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { resolveAliasMap } from '../alias-config.js';

describe('resolveAliasMap', () => {
  let tempDir: string;

  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'lingxia-alias-'));
  });

  afterEach(() => {
    fs.rmSync(tempDir, { recursive: true, force: true });
  });

  it('returns default @ alias when alias undefined', () => {
    const aliases = resolveAliasMap(tempDir);
    expect(Object.keys(aliases)).toContain('@');
    expect(aliases['@']).toBe(tempDir);
  });

  it('normalizes project-relative aliases to absolute paths', () => {
    fs.writeFileSync(
      path.join(tempDir, 'lingxia.config.ts'),
      `
        export default {
          alias: {
            '@shared': 'shared',
            '@lib': './lib'
          }
        };
      `
    );

    const aliases = resolveAliasMap(tempDir);
    expect(aliases['@shared']).toBe(path.resolve(tempDir, 'shared'));
    expect(aliases['@lib']).toBe(path.resolve(tempDir, './lib'));
  });

  it('filters invalid alias entries', () => {
    fs.writeFileSync(
      path.join(tempDir, 'lingxia.config.ts'),
      `
        export default {
          alias: {
            '': 'shared',
            '@ok': ''
          }
        };
      `
    );

    const aliases = resolveAliasMap(tempDir);
    expect(Object.keys(aliases)).toEqual(['@']);
  });
});
