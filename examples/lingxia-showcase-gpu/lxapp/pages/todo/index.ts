import type { TypedStorage } from "@lingxia/types";
import { generateTodoId, validateTodoText, getCurrentTimestamp, type Todo } from "../../shared/lib/todo-utils";
import type { TodoActions, TodoFilter as Filter, TodoSnapshot as Snapshot, TodoPageData, TodoTextRequest, TodoIdRequest, TodoFilterRequest } from '../../shared/todo-types';
interface TodoStorage {
  "todo:state": Snapshot;
  "todo:todos": Todo[];
  "todo:filter": string;
  "todo:lastUpdated": string;
}
const filterValue = (value: unknown): Filter => value === "active" || value === "completed" ? value : "all";
const initial = (): Snapshot => ({ todos: [], currentFilter: "all", lastUpdated: getCurrentTimestamp() });
const initialData: TodoPageData = { ...initial(), loading: true, loadError: '' };

Page({
  data: initialData,
  _storage: null as TypedStorage<TodoStorage> | null,
  _disposed: false,
  _saving: false,
  _loading: false,
  async onLoad() { void (this satisfies TodoActions); await this.reload(); },
  onUnload() { this._disposed = true; },
  _ensureStorage() { return this._storage ?? (this._storage = lx.getStorage<TodoStorage>()); },
  async reload() {
    if (this._saving || this._loading || this._disposed) return;
    this._loading = true;
    this.setData({ loading: true, loadError: "" });
    try {
      const storage = this._ensureStorage();
      let state = await storage.get("todo:state");
      if (!state) {
        const [todos, filter, lastUpdated] = await Promise.all([
          storage.get("todo:todos"), storage.get("todo:filter"), storage.get("todo:lastUpdated"),
        ]);
        state = { todos: todos ?? [], currentFilter: filterValue(filter), lastUpdated: lastUpdated ?? getCurrentTimestamp() };
      }
      if (!Array.isArray(state.todos) || state.todos.some(todo => !todo || typeof todo.id !== "string" || typeof todo.text !== "string" || typeof todo.completed !== "boolean")) {
        throw new Error("Invalid saved tasks");
      }
      if (!this._disposed) this.setData({ ...state, currentFilter: filterValue(state.currentFilter), loading: false });
    } catch {
      if (!this._disposed) this.setData({ loading: false, loadError: "Could not load saved tasks. Retry before making changes." });
    } finally { this._loading = false; }
  },
  async _commit(change: Partial<Snapshot>) {
    if (this._disposed || this._saving || this.data.loading || this.data.loadError) throw new Error("Tasks are not ready");
    this._saving = true;
    try {
      const state: Snapshot = { todos: this.data.todos.map(todo => ({ ...todo })), currentFilter: this.data.currentFilter, lastUpdated: getCurrentTimestamp(), ...change };
      // One write keeps tasks and filter together; failed saves retain the draft and visible state.
      await this._ensureStorage().set("todo:state", state);
      if (!this._disposed) { this.setData({ ...state }); await this.flush(); }
    } finally { this._saving = false; }
  },
  async addTodo({ text }: Partial<TodoTextRequest> = {}) {
    if (!validateTodoText(text)) return;
    await this._commit({ todos: [...this.data.todos, { id: generateTodoId(), text: text.trim(), completed: false }] });
  },
  async toggleTodo({ id }: Partial<TodoIdRequest> = {}) {
    if (!id) return;
    await this._commit({ todos: this.data.todos.map(todo => todo.id === id ? { ...todo, completed: !todo.completed } : todo) });
  },
  async toggleAll() {
    const completed = !this.data.todos.every(todo => todo.completed);
    await this._commit({ todos: this.data.todos.map(todo => ({ ...todo, completed })) });
  },
  async deleteTodo({ id }: Partial<TodoIdRequest> = {}) {
    if (id) await this._commit({ todos: this.data.todos.filter(todo => todo.id !== id) });
  },
  async clearCompleted() { await this._commit({ todos: this.data.todos.filter(todo => !todo.completed) }); },
  async setFilter({ filter }: Partial<TodoFilterRequest> = {}) {
    if (filter && ["all", "active", "completed"].includes(filter)) await this._commit({ currentFilter: filterValue(filter) });
  },
  async goBack() { await lx.navigateBack(); },
});
