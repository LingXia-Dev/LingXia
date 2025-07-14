#!/usr/bin/env node

import { Command } from 'commander';
import { buildCommand } from './commands/build.js';
import { createCommand } from './commands/create.js';

const program = new Command();

program
  .name('lingxia')
  .description('LingXia MiniApp Builder - Build cross-platform mini applications')
  .version('1.0.0');

program
  .command('build')
  .description('Build LingXia project')
  .option('--dev', 'Build in development mode')
  .option('--prod', 'Build in production mode')
  .action(buildCommand);

program
  .command('create <app-name>')
  .description('Create a new LingXia project')
  .action(createCommand);

program.parse();
