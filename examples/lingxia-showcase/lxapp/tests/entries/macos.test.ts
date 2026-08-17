import '../support/aggregate-preflight.test.js';
import './shared.test.js';
import '../platform/desktop/browser-cover-restore.test.js';
import '../platform/desktop/surface-workspace.test.js';
import '../platform/desktop/surface-window.test.js';
import '../platform/desktop/terminal.test.js';
import '../platform/macos/location-permission.test.js';
import { registerContractAudit } from '../support/contract.js';

registerContractAudit({ requireCanonicalShape: true });
