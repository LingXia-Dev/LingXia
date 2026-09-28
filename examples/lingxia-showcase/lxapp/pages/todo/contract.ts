import type { PageContract } from '@lingxia/types/page';
import type { Todo } from '../../shared/lib/todo-utils';

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
