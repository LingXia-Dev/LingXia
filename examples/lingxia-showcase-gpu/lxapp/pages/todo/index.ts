import type { TypedStorage } from '@lingxia/types';
import { generateTodoId, validateTodoText, getCurrentTimestamp, type Todo } from '../../shared/lib/todo-utils';
import type {
  TodoActions,
  TodoFilter as Filter,
  TodoSnapshot as Snapshot,
  TodoPageData,
  TodoTextRequest,
  TodoIdRequest,
  TodoFilterRequest,
} from '../../shared/todo-types';
import { definePage } from '@lingxia/types/page';
interface TodoStorage {
  'todo:state': Snapshot;
  'todo:todos': Todo[];
  'todo:filter': string;
  'todo:lastUpdated': string;
}
const filterValue = (value: unknown): Filter => (value === 'active' || value === 'completed' ? value : 'all');
const initial = (): Snapshot => ({ todos: [], currentFilter: 'all', lastUpdated: getCurrentTimestamp() });
const initialData: TodoPageData = { ...initial(), loading: true, loadError: '' };

export default definePage({
  data: initialData,
  setup(page) {
    let _storage = null as TypedStorage<TodoStorage> | null;
    let disposed = false;
    let saving = false;
    let _loading = false;
    const helpers = {
      ensureStorage() {
        return _storage ?? (_storage = lx.storage<TodoStorage>());
      },
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
          await helpers.ensureStorage().set('todo:state', state);
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
          const storage = helpers.ensureStorage();
          let state = await storage.get('todo:state');
          if (!state) {
            const [todos, filter, lastUpdated] = await Promise.all([
              storage.get('todo:todos'),
              storage.get('todo:filter'),
              storage.get('todo:lastUpdated'),
            ]);
            state = { todos: todos ?? [], currentFilter: filterValue(filter), lastUpdated: lastUpdated ?? getCurrentTimestamp() };
          }
          if (
            !Array.isArray(state.todos) ||
            state.todos.some(todo => !todo || typeof todo.id !== 'string' || typeof todo.text !== 'string' || typeof todo.completed !== 'boolean')
          ) {
            throw new Error('Invalid saved tasks');
          }
          if (!disposed) page.update(d => {
            Object.assign(d, state);
            d.currentFilter = filterValue(state.currentFilter);
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
      async addTodo({ text }: Partial<TodoTextRequest> = {}) {
        if (!validateTodoText(text)) return;
        await helpers.commit({ todos: [...page.data.todos, { id: generateTodoId(), text: text.trim(), completed: false }] });
      },
      async toggleTodo({ id }: Partial<TodoIdRequest> = {}) {
        if (!id) return;
        await helpers.commit({ todos: page.data.todos.map(todo => (todo.id === id ? { ...todo, completed: !todo.completed } : todo)) });
      },
      async toggleAll() {
        const completed = !page.data.todos.every(todo => todo.completed);
        await helpers.commit({ todos: page.data.todos.map(todo => ({ ...todo, completed })) });
      },
      async deleteTodo({ id }: Partial<TodoIdRequest> = {}) {
        if (id) await helpers.commit({ todos: page.data.todos.filter(todo => todo.id !== id) });
      },
      async clearCompleted() {
        await helpers.commit({ todos: page.data.todos.filter(todo => !todo.completed) });
      },
      async setFilter({ filter }: Partial<TodoFilterRequest> = {}) {
        if (filter && ['all', 'active', 'completed'].includes(filter)) await helpers.commit({ currentFilter: filterValue(filter) });
      },
      async goBack() {
        await lx.nav.back();
      },
    };
    return {
      async onLoad() {
        void (actions satisfies TodoActions);
        await actions.reload();
      },
      onUnload() {
        disposed = true;
      },
      actions,
    };
  },
});
