import { definePage, lx } from '@lingxia/logic';
import { generateTodoId, validateTodoText, getCurrentTimestamp, type Todo } from '../../shared/lib/todo-utils';

type Filter = 'all' | 'active' | 'completed';
interface Snapshot { todos: Todo[]; currentFilter: Filter; lastUpdated: string }
const isTodo = (value: unknown): value is Todo => {
  const todo = value as Partial<Todo> | null;
  return !!todo && typeof todo.id === 'string' && typeof todo.text === 'string' && typeof todo.completed === 'boolean';
};
const decodeTodos = (value: unknown): Todo[] => {
  if (!Array.isArray(value) || !value.every(isTodo)) throw new Error('Invalid saved tasks');
  return value;
};
const decodeSnapshot = (value: unknown): Snapshot => {
  const state = (value ?? {}) as Partial<Record<keyof Snapshot, unknown>>;
  return {
    todos: decodeTodos(state.todos),
    currentFilter: filterValue(state.currentFilter),
    lastUpdated: typeof state.lastUpdated === 'string' ? state.lastUpdated : getCurrentTimestamp(),
  };
};
const filterValue = (value: unknown): Filter => (value === 'active' || value === 'completed' ? value : 'all');
const initial = (): Snapshot => ({ todos: [], currentFilter: 'all', lastUpdated: getCurrentTimestamp() });
const initialData = { ...initial(), loading: true, loadError: '' };

export default definePage({
  data: initialData,
  setup(page) {
    let disposed = false;
    let saving = false;
    let _loading = false;
    const helpers = {
      async commit(change: Partial<Snapshot>) {
        if (disposed || saving || page.data.loading || page.data.loadError) throw new Error('Tasks are not ready');
        saving = true;
        try {
          const state: Snapshot = {
            todos: page.data.todos.map(todo => ({ ...todo })),
            currentFilter: page.data.currentFilter,
            lastUpdated: getCurrentTimestamp(),
            ...change,
          };
          // One write keeps tasks and filter together; failed saves retain the draft and visible state.
          await lx.storage.set('todo:state', state);
          if (!disposed) {
            page.update(d => { Object.assign(d, state); });
            await page.flush();
          }
        } finally {
          saving = false;
        }
      },
    };
    const actions = {
      async reload() {
        if (saving || _loading || disposed) return;
        _loading = true;
        page.update(d => {
          d.loading = true;
          d.loadError = '';
        });
        try {
          let state = await lx.storage.get('todo:state', { decode: decodeSnapshot });
          if (!state) {
            const [todos, filter, lastUpdated] = await Promise.all([
              lx.storage.get('todo:todos', { decode: decodeTodos }),
              lx.storage.get('todo:filter'),
              lx.storage.get('todo:lastUpdated'),
            ]);
            state = decodeSnapshot({ todos: todos ?? [], currentFilter: filter, lastUpdated });
          }
          const loaded = state;
          if (!disposed) page.update(d => {
            Object.assign(d, loaded);
            d.loading = false;
          });
        } catch {
          if (!disposed) page.update(d => {
            d.loading = false;
            d.loadError = 'Could not load saved tasks. Retry before making changes.';
          });
        } finally {
          _loading = false;
        }
      },
      async addTodo({ text }: { text?: string } = {}) {
        if (!validateTodoText(text)) return;
        await helpers.commit({ todos: [...page.data.todos, { id: generateTodoId(), text: text.trim(), completed: false }] });
      },
      async toggleTodo({ id }: { id?: string } = {}) {
        if (!id) return;
        await helpers.commit({ todos: page.data.todos.map(todo => (todo.id === id ? { ...todo, completed: !todo.completed } : todo)) });
      },
      async toggleAll() {
        const completed = !page.data.todos.every(todo => todo.completed);
        await helpers.commit({ todos: page.data.todos.map(todo => ({ ...todo, completed })) });
      },
      async deleteTodo({ id }: { id?: string } = {}) {
        if (id) await helpers.commit({ todos: page.data.todos.filter(todo => todo.id !== id) });
      },
      async clearCompleted() {
        await helpers.commit({ todos: page.data.todos.filter(todo => !todo.completed) });
      },
      async setFilter({ filter }: { filter?: Filter } = {}) {
        if (filter && ['all', 'active', 'completed'].includes(filter)) await helpers.commit({ currentFilter: filterValue(filter) });
      },
    };
    return {
      async onLoad() {
        await actions.reload();
      },
      onUnload() {
        disposed = true;
      },
      actions,
    };
  },
});
