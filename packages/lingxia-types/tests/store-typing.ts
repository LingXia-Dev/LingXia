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
