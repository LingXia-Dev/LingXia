import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  DEFAULT_SOURCE_DIRS,
  resolveSourceDirs
} from '../source-dirs.js';

describe('resolveSourceDirs', () => {
  let tempDir: string;

  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'lingxia-source-dirs-'));
  });

  afterEach(() => {
    fs.rmSync(tempDir, { recursive: true, force: true });
  });

  it('returns defaults when config missing', () => {
    expect(resolveSourceDirs(tempDir)).toEqual(DEFAULT_SOURCE_DIRS);
  });

  it('normalizes entries and removes duplicates', () => {
    fs.writeFileSync(
      path.join(tempDir, 'lingxia.config.ts'),
      `
        export default {
          sourceDirs: [' shared ', './shared', 'utils', '']
        };
      `
    );

    expect(resolveSourceDirs(tempDir)).toEqual(['shared', 'utils']);
  });

  it('falls back to defaults when entries invalid', () => {
    fs.writeFileSync(
      path.join(tempDir, 'lingxia.config.ts'),
      `
        export default {
          sourceDirs: ['', 123]
        };
      `
    );

    expect(resolveSourceDirs(tempDir)).toEqual(DEFAULT_SOURCE_DIRS);
  });
});
