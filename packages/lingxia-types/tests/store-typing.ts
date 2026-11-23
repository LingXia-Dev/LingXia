// defineStore: JSON values only, update only for object or array values, picks typed from the value.
import { defineStore, type Store } from '../src/page.js';
import type { Live } from '../src/index.js';

type Item = { id: string; qty: number };
const cart = defineStore({ items: [] as Item[], note: null as string | null });
cart.update(d => { d.items.push({ id: 'a', qty: 1 }); });
cart.update(d => d.items.pop());
cart.set({ items: [], note: 'x' });
// A value read from the store goes back in whole.
cart.set({ ...cart.get(), items: cart.get().items.filter(i => i.qty > 0) });
cart.set(cart.get());
// @ts-expect-error a returned read-only value is not applied
cart.update(() => cart.get());
const count: Live<number> = cart.pick(c => c.items.length);
const items = cart.get().items;
// @ts-expect-error the value is read-only outside update
items.push({ id: 'b', qty: 1 });
// @ts-expect-error a recipe changes the draft in place; a returned value is not applied
cart.update(d => ({ ...d, note: 'y' }));
// @ts-expect-error a recipe is synchronous
cart.update(async d => { d.note = 'z'; });
// @ts-expect-error set takes the whole value
cart.set({ items: [] });

const user = defineStore(null as { name: string } | null, { name: 'user' });
user.set({ name: 'a' });
user.set(null);
// @ts-expect-error a nullable store changes with set, not update
user.update(d => { d.name = 'b'; });

const flag = defineStore(false);
flag.set(true);
// @ts-expect-error a primitive store changes with set
flag.update(d => !d);

// @ts-expect-error a store's value is JSON data
defineStore({ at: new Date() });
// @ts-expect-error a store's value is JSON data
defineStore({ run() {} });

const asLive: Live<{ readonly items: readonly { readonly id: string; readonly qty: number }[]; readonly note: string | null }> = cart;
const typed: Store<boolean> = flag;
void count; void asLive; void typed;

// derive: values typed from each source, a read-only Live.
import { derive } from '../src/page.js';
declare const hostLanguage: Live<string>;
const override = defineStore(null as string | null);
const language = derive([override, hostLanguage], (o, l) => o ?? l);
const lang: Live<string> = language;
const summary = derive([cart, count], (c, n) => `${c.note ?? ''}:${n}`);
const s1: string = summary.get();
// @ts-expect-error a derived value has no set
language.set('en');
// @ts-expect-error each argument has its source's type
derive([count], (n: string) => n);
// @ts-expect-error sources are Live values
derive([1], (n) => n);
void lang; void s1;

// persist: a key, or the app's own load and save; ready, reload and saved only on a persisted store.
import type { PersistResult, PersistedStore } from '../src/page.js';
type Session = { access: string; serviceUrl: string } | null;
const sessionStore = defineStore<Session>(null, {
  persist: {
    async load() { return { access: 'a', serviceUrl: 'u' }; },
    async save(value) { const access: string | undefined = value?.access; void access; },
  },
});
const persisted: PersistedStore<Session> = sessionStore;
const readyResult: Promise<PersistResult> = sessionStore.ready;
const lastSave: Live<PersistResult> = sessionStore.saved;
const theme = defineStore('light' as 'light' | 'dark', { persist: { key: 'theme' } });
theme.set('dark');
void theme.reload();
// @ts-expect-error a store without persist has no ready
void cart.ready;
// @ts-expect-error load resolves the store's value
defineStore<Session>(null, {
  persist: { async load() { return 1; }, async save() {} },
});
// @ts-expect-error persist is a key or load and save
defineStore(0, { persist: { load: async () => 0 } });
void persisted; void readyResult; void lastSave;
