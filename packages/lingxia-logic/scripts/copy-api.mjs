// The API catalogue ships here too, so `lingxia api` finds it in a project that
// installs only this package, wherever npm put @lingxia/types. So does the page
// runtime, which `mount` from `@lingxia/logic/testing` runs in Node.
import { copyFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = join(dirname(fileURLToPath(import.meta.url)), '..');
// The checked-in catalogue at the types package root: no build of it is needed first.
let dir = here;
let source;
while (!source) {
  const candidate = join(dir, 'node_modules', '@lingxia', 'types', 'api.json');
  if (existsSync(candidate)) source = candidate;
  else if (dirname(dir) === dir) throw new Error('@lingxia/types api.json not found');
  else dir = dirname(dir);
}
copyFileSync(source, join(here, 'api.json'));
copyFileSync(join(here, '..', '..', 'crates', 'lingxia-lxapp', 'src', 'appservice', 'scripts', 'Page.js'), join(here, 'page-runtime.js'));
