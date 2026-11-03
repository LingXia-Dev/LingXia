import { trackPublicSurface } from '@lingxia/test/runner';
import '../setup.js';

// The Showcase is LingXia's conformance suite: it intends to reach every
// published capability, so the report measures the whole surface and shows
// what it has not reached. An ordinary lxapp leaves this off.
trackPublicSurface();

import '../api/automation.test.js';
import '../api/network-route.test.js';
import '../api/mocks.test.js';
import '../api/clock.test.js';
import '../api/applink.test.js';
import '../api/surface.test.js';
import '../api/runtime.test.js';
import '../api/navigation.test.js';
import '../api/navigation-app.test.js';
import '../api/device-profile.test.js';
import '../api/io-contracts.test.js';
import '../api/host-app.test.js';
import '../api/argument-contracts.test.js';
import '../api/clipboard.test.js';
import '../api/transfer.test.js';
import '@lingxia/test/showcase/pages/bridge-repro.test.js';
import '@lingxia/test/showcase/pages/stream.test.js';
import '@lingxia/test/showcase/pages/surface-port.test.js';
import '@lingxia/test/showcase/pages/video-playback.test.js';
import '@lingxia/test/showcase/pages/media-info.test.js';
import '@lingxia/test/showcase/pages/channel.test.js';
import '@lingxia/test/showcase/pages/cloud.test.js';
import '@lingxia/test/showcase/pages/components.test.js';
import '@lingxia/test/showcase/pages/navigator.test.js';
import '@lingxia/test/showcase/pages/native-components.test.js';
import '@lingxia/test/showcase/pages/device.test.js';
import '@lingxia/test/showcase/pages/home.test.js';
import '@lingxia/test/showcase/pages/lifecycle.test.js';
import '@lingxia/test/showcase/pages/page-globals.test.js';
import '@lingxia/test/showcase/pages/pull-to-refresh.test.js';
import '@lingxia/test/showcase/pages/system.test.js';
import '@lingxia/test/showcase/pages/todo.test.js';
import '@lingxia/test/showcase/pages/ui.test.js';
import '@lingxia/test/showcase/pages/chrome.test.js';
import '@lingxia/test/showcase/pages/clipboard.test.js';
import '@lingxia/test/showcase/pages/leave-guard.test.js';
import '@lingxia/test/showcase/pages/host-chrome.test.js';
import '@lingxia/test/showcase/pages/orientation.test.js';
import '@lingxia/test/showcase/pages/render.test.js';
import '../pending/backlog-pending.test.js';

export {};
