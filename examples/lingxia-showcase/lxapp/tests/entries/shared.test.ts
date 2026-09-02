import { trackPublicSurface } from '@lingxia/test';
import '../setup.js';

// The Showcase is LingXia's conformance suite: it intends to reach every
// published capability, so the report measures the whole surface and shows
// what it has not reached. An ordinary lxapp leaves this off.
trackPublicSurface();

import '../api/automation.test.js';
import '../api/surface.test.js';
import '../api/runtime.test.js';
import '../api/navigation.test.js';
import '../api/io-contracts.test.js';
import '../api/host-app.test.js';
import '../api/argument-contracts.test.js';
import '../api/transfer.test.js';
import '../pages/bridge-repro.test.js';
import '../pages/stream.test.js';
import '../pages/surface-port.test.js';
import '../pages/video-playback.test.js';
import '../pages/channel.test.js';
import '../pages/components.test.js';
import '../pages/native-components.test.js';
import '../pages/device.test.js';
import '../pages/home.test.js';
import '../pages/lifecycle.test.js';
import '../pages/pull-to-refresh.test.js';
import '../pages/system.test.js';
import '../pages/todo.test.js';
import '../pages/ui.test.js';
import '../pages/chrome.test.js';
import '../pages/render.test.js';
import '../pending/backlog-pending.test.js';

export {};
