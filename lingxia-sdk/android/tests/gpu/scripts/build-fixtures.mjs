import {buildPages} from '../../../../../packages/lingxia-ui/compiler.mjs';
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../fixtures/contracts/',import.meta.url));
const output=fileURLToPath(new URL('../../../../../target/gpu-device-fixtures/',import.meta.url));
await buildPages(root,['pages/home/index.ui.tsx'],output);
console.log(`Built device contract fixture: ${output}`);
