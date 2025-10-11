#!/usr/bin/env node

import { Command } from 'commander';
import { buildCommand } from './commands/build.js';

export { buildCommand };

const program = new Command();

program
  .name('lingxia')
  .description('LingXia Build Tool - Build LingXia LxApp projects')
  .version('1.0.0');

program
  .command('build')
  .description('Build LingXia project')
  .option('--dev', 'Build in development mode')
  .option('--prod', 'Build in production mode')
  .action(buildCommand);

program.parse();
