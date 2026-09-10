import '../support/aggregate-preflight.test.js';
import './shared.test.js';
import '../platform/desktop/browser-cover-restore.test.js';
import '../platform/desktop/surface-workspace.test.js';
import '../platform/desktop/surface-window.test.js';
import '../platform/desktop/surface-tab.test.js';
import '../platform/desktop/video-fullscreen.test.js';
// Native https preview opens an overlay. Keep it after in-page video
// fullscreen so a leftover panel cannot steal `play()`.
import '../pages/preview-https.test.js';
import '../platform/desktop/terminal-api.test.js';
import '../platform/desktop/preview-media.test.js';
import '../platform/desktop/terminal.test.js';
import '../platform/macos/location-permission.test.js';

