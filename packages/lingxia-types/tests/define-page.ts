import { definePage, type, type ParamsOf, type ResultOf } from '../src/page.js';
import type { PageDefinition } from '../src/page.js';

type Order = { id: string; total: number };

const order = definePage({
  params: type<{ id: string }>(),
  result: type<{ status: 'open' | 'done' }>(),
  data: { order: null as Order | null, n: 0 },
  setup(page) {
    let timer: number | undefined;
    return {
      onLoad({ id }) {
        const known: string = id;
        page.update(d => { d.order = { id: known, total: 0 }; });
      },
      onUnload() { if (timer !== undefined) clearInterval(timer); },
      actions: {
        inc() { page.update(d => { d.n += 1; }); },
        async markDone() { await page.close({ status: 'done' }); },
        rename(name: string) { return name.length; },
      },
    };
  },
});

// The definition is a page contract: View and tests read data and actions.
const asContract: PageDefinition<{ order: Order | null; n: number }, unknown> = order;
// Actions are Promises to the View.
const counted: (name: string) => Promise<number> = order.actions.rename;
type OrderParams = ParamsOf<typeof order>;
const params: OrderParams = { id: 'a' };
const done: ResultOf<typeof order> = { status: 'done' };

definePage({
  data: { n: 0, list: [] as string[], rows: [] as { id: string }[] },
  setup: page => ({
    actions: {
      bad() {
        // @ts-expect-error a recipe is synchronous
        page.update(async d => { d.n = 1; });
        // @ts-expect-error a recipe changes the draft in place
        page.update(d => ({ ...d, n: 1 }));
        page.update(d => d.n++);
        page.update(d => d.list.push('x'));
        page.update(d => d.list.splice(0, 1));
        page.update(d => d.rows.pop());
        // @ts-expect-error data is readonly
        page.data.n = 2;
      },
    },
  }),
});

definePage({
  result: type<{ status: 'done' }>(),
  setup: page => ({
    actions: {
      // @ts-expect-error close takes the declared result
      wrong() { return page.close({ status: 'nope' }); },
    },
  }),
});

definePage({
  // @ts-expect-error put View-callable functions under actions
  setup: () => ({ save() {} }),
});

definePage({
  // @ts-expect-error a key a case away from a hook
  setup: () => ({ onUnLoad() {} }),
});

definePage({
  // @ts-expect-error setup is synchronous
  setup: async () => ({}),
});

definePage({
  // @ts-expect-error data is JSON
  data: { at: new Date() },
});

export type DefinePageGate = [typeof asContract, typeof counted, typeof params, typeof done];

definePage({
  setup(page) {
    const compact: boolean = page.env.get().sizeClass === 'compact';
    const stop: () => void = page.env.watch(({ sizeClass, canDock }) => {
      void sizeClass;
      void canDock;
    });
    // @ts-expect-error pixels stay in the View's env
    page.env.get().width;
    // @ts-expect-error the env is read with get()
    page.env.sizeClass;
    return { onUnload: () => stop(), onLoad: () => void compact };
  },
});

const chat = definePage({
  messages: type<{ in: { text: string }; out: { read: number } }>(),
  setup(page) {
    return {
      onLoad() {
        page.opener?.onMessage((message) => {
          const text: string = message.text;
          page.opener?.send({ read: text.length });
        });
        // @ts-expect-error the page sends `out`, not `in`
        page.opener?.send({ text: 'x' });
      },
    };
  },
});
void chat;

const freeform = definePage({
  setup(page) {
    return { onLoad() { page.opener?.send({ anything: true }); } };
  },
});
void freeform;

// The short form: `actions: page => ({ … })`, typed from `data` like `setup`.
const counter = definePage({
  params: type<{ start: number }>(),
  data: { count: 0 },
  actions: page => {
    let calls = 0;
    return {
      bump(by: number) { calls += 1; page.update(d => { d.count += by; }); return calls; },
      start() { return page.params.start; },
    };
  },
});
const bumped: (by: number) => Promise<number> = counter.actions.bump;
const startParams: ParamsOf<typeof counter> = { start: 1 };
// @ts-expect-error bump takes a number
counter.actions.bump('x');

// A plain object is the same, without the page.
const plain = definePage({ data: { n: 0 }, actions: { ping(): 'pong' { return 'pong'; } } });
const pinged: () => Promise<'pong'> = plain.actions.ping;

definePage({
  data: { n: 0 },
  // @ts-expect-error setup and actions together
  setup: () => ({}),
  actions: { ping() {} },
});

definePage({
  data: { n: 0 },
  actions: page => ({
    bad() {
      // @ts-expect-error data is readonly
      page.data.n = 1;
    },
  }),
});

// @ts-expect-error actions(page) is synchronous
definePage({ actions: async () => ({ go() {} }) });
// A this-less action reads no page: `this` is void.
definePage({ actions: { who() { return this; } } }).actions.who satisfies () => Promise<void>;

definePage({
  setup(page) {
    return {
      onShow: () => page.orientation.lock('landscape'),
      onHide: () => page.orientation.unlock(),
      onLoad: () => {
        // @ts-expect-error a page locks to portrait or landscape only
        page.orientation.lock('sideways');
      },
    };
  },
});
