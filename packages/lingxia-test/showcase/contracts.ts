// The page contracts the shared specs drive; each showcase implements them.
import type { PageContract } from '@lingxia/types/page';

export interface Todo {
  id: string;
  text: string;
  completed: boolean;
}

export type TodoFilter = 'all' | 'active' | 'completed';

export type TodoPage = PageContract<
  { todos: Todo[]; currentFilter: TodoFilter; lastUpdated: string },
  {
    addTodo(params: { text: string }): Promise<void>;
    toggleTodo(params: { id: string }): Promise<void>;
    deleteTodo(params: { id: string }): Promise<void>;
    clearCompleted(): Promise<void>;
    setFilter(params: { filter: TodoFilter }): Promise<void>;
  }
>;

/** A stream page chat message, as the specs read it. */
export interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
}
