import { Command } from 'commander';
import { createRequire } from 'module';
import { buildCommand } from './builder/index.js';
import { createCommand } from './commands/create.js';

const { version } = createRequire(import.meta.url)('../package.json');

export function runCLI(): void {
  const program = new Command();
  program
    .name('lingxia')
    .description('LingXia CLI - Build LingXia LxApp projects')
    .version(version ?? '0.0.0');

  program
    .command('build')
    .description('Build LingXia project or plugin')
    .option('-d, --dev', 'Development build')
    .option('-p, --prod', 'Production build')
    .option('--plugin', 'Build as plugin package')
    .option('--target <target>', 'JS target (es5, es2015, es2020, esnext). Note: es5 requires @vitejs/plugin-legacy')
    .action(buildCommand);

  program
    .command('create')
    .argument('[projectName]', 'Directory name for the new project')
    .description('Create a new LingXia project from a template')
    .option('-f, --framework <framework>', 'Pick a framework (react|vue)')
    .action((projectName: string | undefined, cmdOptions: { framework?: string }) =>
      createCommand(projectName, {
        framework: cmdOptions.framework as any
      })
    );

  program.parse();
}
