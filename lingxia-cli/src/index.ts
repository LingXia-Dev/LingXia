import { Command } from 'commander';
import { buildCommand } from 'lingxia-builder';

export function runCLI(): void {
  const program = new Command();
  program
    .name('lingxia')
    .description('LingXia CLI')
    .version('0.0.0');

  program
    .command('build')
    .description('Build LingXia project')
    .option('-d, --dev', 'Development build')
    .option('-p, --prod', 'Production build')
    .action(buildCommand);

  program.parse();
}
