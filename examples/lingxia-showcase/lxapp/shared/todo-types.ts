export interface Todo { id: string; text: string; completed: boolean }
export type TodoFilter = 'all' | 'active' | 'completed';
export interface TodoSnapshot { todos: Todo[]; currentFilter: TodoFilter; lastUpdated: string }
export interface TodoPageData extends TodoSnapshot { loading: boolean; loadError: string }
export interface TodoTextRequest { text: string }
export interface TodoIdRequest { id: string }
export interface TodoFilterRequest { filter: TodoFilter }
export interface TodoActions {
  reload(): Promise<void>;
  addTodo(request: TodoTextRequest): Promise<void>;
  toggleTodo(request: TodoIdRequest): Promise<void>;
  deleteTodo(request: TodoIdRequest): Promise<void>;
  setFilter(request: TodoFilterRequest): Promise<void>;
  toggleAll(): Promise<void>;
  clearCompleted(): Promise<void>;
}
