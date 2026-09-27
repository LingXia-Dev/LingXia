// The Showcase's mock handlers: one per call they answer, kept for good so
// any call can switch to mocks. Development only — product code never
// imports this directory (every build fails if it does); the app calls
// `fetch`, and in dev the mock selection (`mocks/config.json`,
// `lingxia dev --mock`, `lxdev mock`) decides whether a handler here or the
// real backend answers.
import type { Mocks } from '@lingxia/types/mocks';
import { PROFILE } from './fixtures';

// Module state lasts until the next save under mocks/, `lxdev mock reset`,
// an app restart, or the next spec.
let visits = 0;
let name = PROFILE.name;

export default {
  'GET https://api.example.com/lingxia-showcase/mocks/profile': () => {
    visits += 1;
    return { json: { ...PROFILE, name, visits } };
  },
  'PATCH https://api.example.com/lingxia-showcase/mocks/profile': async (req) => {
    const body = await req.json<{ name?: string }>();
    if (!body.name) return { status: 422, json: { error: 'name_required' } };
    name = body.name;
    return { json: { ...PROFILE, name, visits } };
  },
  'GET https://api.example.com/lingxia-showcase/mocks/broken': () => {
    throw new Error('the handler broke');
  },
} satisfies Mocks;
