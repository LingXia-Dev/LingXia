import type { Mocks } from '../src/mocks.js';

// What the generated native client declares for an lxapp's routes.
declare module '../src/mocks.js' {
  interface NativeMockRegistry {
    routes: {
      'notes.list': { input: undefined; result: { id: number }[]; event: never; error: { code: 'E_INTERNAL' } };
      'notes.delete': {
        input: { id: number };
        result: null;
        event: never;
        error: { code: 'NOT_FOUND'; data: { id: number } } | { code: 'LOCKED' };
      };
      'notes.import': {
        input: { from: string };
        result: { imported: number };
        event: { done: number; total: number };
        error: { code: 'E_INTERNAL' };
      };
    };
  }
}

const mocks = {
  'GET **/devices': (req) => ({ json: { method: req.method } }),
  'native notes.list': () => [{ id: 1 }],
  'native notes.delete': ({ id }, ctx) => {
    if (id === 7) throw ctx.error('NOT_FOUND', { id });
    return null;
  },
  'native notes.import': async function* ({ from }) {
    yield { done: 1, total: 2 };
    return { imported: from.length };
  },
  // A glob may match routes of any type.
  'native tags.*': (input, ctx) => (ctx.route === 'tags.list' ? [] : input),
  'native flags.list': ['a'],
} satisfies Mocks;
void mocks;

// @ts-expect-error the result is the route's
const wrongResult = { 'native notes.list': () => [{ id: 'x' }] } satisfies Mocks;
// @ts-expect-error NOT_FOUND carries an id
const wrongData = { 'native notes.delete': (_input, ctx) => { throw ctx.error('NOT_FOUND', { name: 'x' }); } } satisfies Mocks;
void wrongData;
// @ts-expect-error NOPE is not one of the route's errors
const wrongError = { 'native notes.delete': (_input, ctx) => { throw ctx.error('NOPE'); } } satisfies Mocks;
// @ts-expect-error an event is the route's
const wrongEvent = { 'native notes.import': function* () { yield 'x'; return { imported: 1 }; } } satisfies Mocks;
void [wrongResult, wrongError, wrongEvent];
