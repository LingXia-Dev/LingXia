import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  defineLingxiaConfig,
  extractPluginSpecs,
  loadLingxiaConfig
} from '../lingxia-config.js';

describe('lingxia-config loader', () => {
  let tempDir: string;

  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'lingxia-config-'));
  });

  afterEach(() => {
    fs.rmSync(tempDir, { recursive: true, force: true });
  });

  it('returns undefined when no config file exists', () => {
    expect(loadLingxiaConfig(tempDir)).toBeUndefined();
  });

  it('loads the exported configuration object', () => {
    fs.writeFileSync(
      path.join(tempDir, 'lingxia.config.ts'),
      `
        export default ${JSON.stringify(
          defineLingxiaConfig({ staticDirs: ['media'] })
        )};
      `
    );

    expect(loadLingxiaConfig(tempDir)).toEqual({ staticDirs: ['media'] });
  });

  it('ignores invalid exports', () => {
    fs.writeFileSync(
      path.join(tempDir, 'lingxia.config.ts'),
      `export default 42;`
    );

    expect(loadLingxiaConfig(tempDir)).toBeUndefined();
  });

  it('defineLingxiaConfig returns the provided config', () => {
    const config = { staticDirs: ['foo'] };
    expect(defineLingxiaConfig(config)).toBe(config);
  });

  it('normalizes plugin specs from config object', () => {
    const config = defineLingxiaConfig({
      plugins: {
        react: [
          'vite-plugin-one',
          { module: './plugins/local-plugin.ts', namedExport: 'createPlugin', options: { foo: true } },
          { module: '' }
        ]
      }
    });

    const specs = extractPluginSpecs(config);
    expect(specs?.react).toEqual([
      { module: 'vite-plugin-one' },
      {
        module: './plugins/local-plugin.ts',
        namedExport: 'createPlugin',
        options: { foo: true }
      }
    ]);
    expect(specs?.vue).toBeUndefined();
  });

  it('applies shared plugin array to all frameworks', () => {
    const sharedPlugin = { name: 'shared-plugin', transform() {} };
    const config = defineLingxiaConfig({
      plugins: ['vite-plugin-shared', sharedPlugin]
    });

    const specs = extractPluginSpecs(config);
    expect(specs?.react).toEqual([
      { module: 'vite-plugin-shared' },
      { plugin: sharedPlugin }
    ]);
    expect(specs?.vue).toEqual([
      { module: 'vite-plugin-shared' },
      { plugin: sharedPlugin }
    ]);
  });
});
