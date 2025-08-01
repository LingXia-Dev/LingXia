#!/usr/bin/env node

/**
 * LingXia CLI Tool
 * Build tool for LingXia LxApp development with smart dependency resolution and TypeScript support
 */

import { program } from 'commander';
import { fileURLToPath } from 'url';
import { dirname, resolve } from 'path';
import { readFileSync } from 'fs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Read version information
const packagePath = resolve(__dirname, '../package.json');
const packageInfo = JSON.parse(readFileSync(packagePath, 'utf-8'));

program
  .name('lingxia')
  .description('LingXia Build Tool - Build tool for LingXia LxApp development')
  .version(packageInfo.version);

// build command
program
  .command('build')
  .description('Build the LxApp project')
  .option('-d, --dev', 'Development build')
  .option('-p, --prod', 'Production build')
  .option('--no-cleanup', 'Keep build directory for debugging')
  .option('--output <dir>', 'Output directory', 'dist')
  .action(async (options) => {
    const { buildCommand } = await import('../dist/commands/build.js');
    await buildCommand(options);
  });

// Parse command line arguments
program.parse();
