import path from 'path';
import { fileURLToPath } from 'url';
import { existsSync, promises as fs } from 'fs';
import { createInterface } from 'readline/promises';
import { stdin as input, stdout as output } from 'process';

type Framework = 'react' | 'vue';

type CreateOptions = {
  framework?: Framework;
};

const TEMPLATE_ROOT = fileURLToPath(new URL('../../templates/create', import.meta.url));

export async function createCommand(projectNameArg?: string, options: CreateOptions = {}): Promise<void> {
  const prompts = new PromptManager();
  try {
    const projectName = await resolveProjectName(projectNameArg, prompts);
    const framework = await resolveFramework(options.framework, prompts);

    const targetDir = path.resolve(process.cwd(), projectName);
    await ensureTargetDirectory(targetDir);

    const templateDir = path.join(TEMPLATE_ROOT, framework);
    await copyTemplate(templateDir, targetDir);

    await applyPlaceholders(targetDir, projectName);
    await recordFrameworkMetadata(targetDir, framework);

    console.log('\n✅ Project scaffolded!');
    console.log(`\nNext steps:`);
    console.log(`  cd ${path.relative(process.cwd(), targetDir) || '.'}`);
    console.log(`  npm install`);
    console.log(`  npm run dev # or npm run build`);
  } finally {
    prompts.dispose();
  }
}

async function resolveProjectName(projectNameArg: string | undefined, prompts: PromptManager): Promise<string> {
  const defaultName = 'lingxia-app';
  if (projectNameArg && projectNameArg.trim().length > 0) {
    return projectNameArg.trim();
  }
  return prompts.input(`Project name (${defaultName}): `, defaultName);
}

async function resolveFramework(current: string | undefined, prompts: PromptManager): Promise<Framework> {
  if (current === 'react' || current === 'vue') {
    return current;
  }
  const answer = await prompts.select('Choose framework', ['react', 'vue']);
  return answer as Framework;
}

async function ensureTargetDirectory(targetDir: string): Promise<void> {
  if (existsSync(targetDir)) {
    const entries = await fs.readdir(targetDir);
    if (entries.length > 0) {
      throw new Error(`Target directory "${targetDir}" is not empty. Please choose a different name or empty the folder.`);
    }
    return;
  }
  await fs.mkdir(targetDir, { recursive: true });
}

async function copyTemplate(templateDir: string, targetDir: string): Promise<void> {
  await fs.cp(templateDir, targetDir, { recursive: true });
}

async function applyPlaceholders(targetDir: string, projectName: string): Promise<void> {
  const pkgPath = path.join(targetDir, 'package.json');
  const appPath = path.join(targetDir, 'lxapp.json');

  const packageName = slugify(projectName);
  const appId = packageName;
  const displayName = titleCase(projectName);

  const replacements: Record<string, string> = {
    __APP_PACKAGE_NAME__: packageName,
    __APP_ID__: appId,
    __APP_DISPLAY_NAME__: displayName
  };

  await replacePlaceholders(pkgPath, replacements);
  await replacePlaceholders(appPath, replacements);
}

async function recordFrameworkMetadata(targetDir: string, framework: Framework): Promise<void> {
  const pkgPath = path.join(targetDir, 'package.json');
  if (!existsSync(pkgPath)) return;
  const pkg = JSON.parse(await fs.readFile(pkgPath, 'utf8'));
  pkg.lingxia = { ...(pkg.lingxia ?? {}), framework };
  await fs.writeFile(pkgPath, JSON.stringify(pkg, null, 2));
}

async function replacePlaceholders(filePath: string, replacements: Record<string, string>): Promise<void> {
  if (!existsSync(filePath)) return;
  let content = await fs.readFile(filePath, 'utf8');
  for (const [token, value] of Object.entries(replacements)) {
    content = content.replace(new RegExp(token, 'g'), value);
  }
  await fs.writeFile(filePath, content);
}

class PromptManager {
  private rl: ReturnType<typeof createInterface> | null = null;

  private ensureInterface() {
    if (!this.rl) {
      this.rl = createInterface({ input, output });
    }
    return this.rl;
  }

  async input(message: string, fallback: string): Promise<string> {
    const answer = await this.ensureInterface().question(message);
    return answer.trim().length > 0 ? answer.trim() : fallback;
  }

  async select(message: string, options: string[]): Promise<string> {
    const formatted = options.map((option, index) => `${index + 1}) ${option}`).join('\n');
    const prompt = `${message}:\n${formatted}\nEnter choice: `;
    const answer = await this.ensureInterface().question(prompt);
    const index = Number.parseInt(answer, 10);
    if (!Number.isNaN(index) && index >= 1 && index <= options.length) {
      return options[index - 1];
    }
    return options[0];
  }

  async confirm(message: string, defaultValue: boolean): Promise<boolean> {
    const suffix = defaultValue ? 'Y/n' : 'y/N';
    const answer = (await this.ensureInterface().question(message || `Confirm (${suffix}): `)).trim().toLowerCase();
    if (answer === '') {
      return defaultValue;
    }
    return ['y', 'yes'].includes(answer);
  }

  dispose() {
    this.rl?.close();
    this.rl = null;
  }
}

function slugify(value: string): string {
  const slug = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return slug.length > 0 ? slug : 'lingxia-app';
}

function titleCase(value: string): string {
  return value
    .trim()
    .split(/[\s\-_.]+/)
    .filter(Boolean)
    .map(word => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ') || 'LingXia App';
}
