import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  ViewConfigManager,
  resolveUserViewConfig
} from '../view-config.js';

describe('ViewConfigManager integration', () => {
  let tempDir: string;

  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'lingxia-ui-config-'));
  });

  afterEach(() => {
    fs.rmSync(tempDir, { recursive: true, force: true });
  });

  it('returns undefined overrides when no config file exists', () => {
    expect(resolveUserViewConfig(tempDir, 'react')).toBeUndefined();
  });

  it('loads overrides from lingxia.config.ts and merges with defaults', () => {
    fs.writeFileSync(
      path.join(tempDir, 'lingxia.config.ts'),
      `
        export default {
          view: {
            react: {
              output: {
                multi: { entryFileNames: 'bundle.js' }
              },
              minifyStrategy: false
            }
          }
        };
      `
    );

    const overrides = resolveUserViewConfig(tempDir, 'react');
    expect(overrides?.react?.output?.multi?.entryFileNames).toBe('bundle.js');
    expect(overrides?.react?.minifyStrategy).toBe(false);
    expect(overrides?.vue).toBeUndefined();

    const manager = new ViewConfigManager(tempDir, overrides);
    const config = manager.getFrameworkConfig('react');
    expect(config.output.multi.entryFileNames).toBe('bundle.js');
    expect(config.minifyStrategy).toBe(false);
    // untouched defaults remain
    expect(config.output.multi.assetFileNames).toBe('assets/[name].[ext]');
  });
});
