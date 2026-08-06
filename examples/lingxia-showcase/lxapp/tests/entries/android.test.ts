import '../support/aggregate-preflight.test.js';
import './shared.test.js';
import { registerContractAudit } from '../support/contract.js';

registerContractAudit({ requireCanonicalShape: true });
