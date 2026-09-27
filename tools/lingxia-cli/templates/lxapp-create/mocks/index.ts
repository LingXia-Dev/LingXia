// Mock handlers: one per call this app makes, kept for good so any call can
// switch to a mock. Keys are HTTP targets ('METHOD url-glob'); values are an
// answer or a handler `(req, ctx) => answer`. In development the selection
// decides who answers: mocks/config.json, `lingxia dev --mock all|none`, or
// `lxdev mock`. Development only: product code never imports this directory,
// and every build fails if it does.
import type { Mocks } from '@lingxia/types/mocks';

export default {
  'GET https://api.example.com/greeting': (req) => ({
    json: { text: `Hello from a mock (${req.method} ${req.url.pathname})` },
  }),
} satisfies Mocks;
