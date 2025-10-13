import { Command } from 'commander';
import { buildCommand } from 'lingxia-builder';

export function runCLI(): void {
  const program = new Command();
  program
    .name('lingxia')
    .description('LingXia CLI - Build LingXia LxApp projects')
    .version('0.0.1');

  program
    .command('build')
    .description('Build LingXia project')
    .option('-d, --dev', 'Development build')
    .option('-p, --prod', 'Production build')
    .action(buildCommand);

  program.parse();
}
