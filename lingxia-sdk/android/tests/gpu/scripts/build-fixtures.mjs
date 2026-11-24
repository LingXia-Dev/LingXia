import {buildPages} from '../../../../../packages/lingxia-view/compiler/index.mjs';
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../fixtures/contracts/',import.meta.url));
const output=fileURLToPath(new URL('../../../../../target/gpu-device-fixtures/',import.meta.url));
const inputsRecord=fileURLToPath(new URL('../../../../../target/gpu-device-fixtures.gpu-inputs.json',import.meta.url));
await buildPages(root,['pages/home/index.tsx'],output,{},{messages:null,inputsRecord});
console.log(`Built device contract fixture: ${output}`);
