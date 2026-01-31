import React from 'react';
import '../../tailwind.css';
import './index.css';

type Todo = {
  id: string;
  text: string;
  completed: boolean;
};

type TodoFilter = 'all' | 'active' | 'completed';

type PageData = {
  todos?: Todo[];
  currentFilter?: TodoFilter;
};

type PageActions = {
  data: PageData;
  addTodo(params: { text: string }): void;
  toggleTodo(params: { id: string }): void;
  deleteTodo(params: { id: string }): void;
  clearCompleted(): void;
  setFilter(params: { filter: TodoFilter }): void;
};

declare function useLingXia(): PageActions;

export default function TodoPage() {
  const {
    data,
    addTodo,
    toggleTodo,
    deleteTodo,
    clearCompleted,
    setFilter,
  } = useLingXia();
  const todos = data?.todos ?? [];
  const currentFilter = data?.currentFilter ?? 'all';

  const [newTodo, setNewTodo] = React.useState('');

  const filteredTodos = React.useMemo(() => {
    switch (currentFilter) {
      case 'active':
        return todos.filter(todo => !todo.completed);
      case 'completed':
        return todos.filter(todo => todo.completed);
      default:
        return todos;
    }
  }, [todos, currentFilter]);

  const todoStats = React.useMemo(() => {
    const completedCount = todos.filter(todo => todo.completed).length;
    return {
      total: todos.length,
      completed: completedCount,
      active: todos.length - completedCount,
    };
  }, [todos]);

  const handleAddTodo = React.useCallback(() => {
    const text = newTodo.trim();
    if (!text) {
      return;
    }
    addTodo({ text });
    setNewTodo('');
  }, [newTodo, addTodo]);

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') {
      handleAddTodo();
    }
  };

  const handleToggleAll = React.useCallback(() => {
    const allCompleted = todoStats.active === 0;
    todos.forEach(todo => {
      if (todo.completed !== !allCompleted) {
        toggleTodo({ id: todo.id });
      }
    });
  }, [todos, todoStats.active, toggleTodo]);

  const handleFilterClick = (filter: TodoFilter) => (
    event: React.MouseEvent<HTMLAnchorElement>
  ) => {
    event.preventDefault();
    setFilter({ filter });
  };

  const hasTodos = todos.length > 0;

  return (
    <div className="todo-page">
      <section className="todoapp">
        <h1>todos</h1>
      <input
        className="new-todo"
        placeholder="What needs to be done?"
        value={newTodo}
        onChange={event => setNewTodo(event.target.value)}
        onKeyDown={handleKeyDown}
        autoFocus
      />

      {!hasTodos && (
        <div className="empty-state">
          <div className="empty-state-icon">📝</div>
          <div className="empty-state-text">No tasks yet</div>
          <div className="empty-state-hint">Add a task above to get started</div>
        </div>
      )}

      {hasTodos && (
        <div className="main">
          <input
            id="toggle-all"
            className="toggle-all"
            type="checkbox"
            checked={todoStats.active === 0 && todos.length > 0}
            onChange={handleToggleAll}
          />
          <label htmlFor="toggle-all">Mark all as complete</label>
          <ul className="todo-list">
            {filteredTodos.map(todo => (
              <li key={todo.id} className={todo.completed ? 'completed' : ''}>
                <div className="view">
                  <input
                    className="toggle"
                    type="checkbox"
                    checked={todo.completed}
                    onChange={() => toggleTodo({ id: todo.id })}
                  />
                  <label>{todo.text}</label>
                  <button
                    className="destroy"
                    onClick={() => deleteTodo({ id: todo.id })}
                    aria-label="Delete todo"
                  />
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}

      {hasTodos && (
        <footer className="footer">
          <span className="todo-count">
            <strong>{todoStats.active}</strong>{' '}
            {todoStats.active === 1 ? 'item' : 'items'} left
          </span>
          <ul className="filters">
            <li>
              <a
                href="#/"
                className={currentFilter === 'all' ? 'selected' : ''}
                onClick={handleFilterClick('all')}
              >
                All
              </a>
            </li>
            <li>
              <a
                href="#/active"
                className={currentFilter === 'active' ? 'selected' : ''}
                onClick={handleFilterClick('active')}
              >
                Active
              </a>
            </li>
            <li>
              <a
                href="#/completed"
                className={currentFilter === 'completed' ? 'selected' : ''}
                onClick={handleFilterClick('completed')}
              >
                Completed
              </a>
            </li>
          </ul>
          {todoStats.completed > 0 && (
            <button className="clear-completed" onClick={clearCompleted}>
              Clear completed
            </button>
          )}
        </footer>
      )}
      </section>
    </div>
  );
}
