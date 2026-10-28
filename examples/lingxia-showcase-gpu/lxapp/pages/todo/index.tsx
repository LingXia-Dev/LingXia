import { List, useLocal, type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, EmptyState, ErrorState, Icon, Segmented } from '../../shared/ui';
import type { Todo, TodoActions, TodoFilter, TodoPageData } from '../../shared/todo-types';

const filters = [
  { value: 'all', label: 'All' },
  { value: 'active', label: 'Active' },
  { value: 'completed', label: 'Completed' },
] as const;

function TodoRow({ todo, blocked, onToggle, onDelete }: { todo: Todo; blocked: boolean; onToggle: () => unknown; onDelete: () => unknown }) {
  return (
    <row testId={`todo-item-${todo.id}`} className="items-center min-h-12 px-3 gap-2 border-t border-line">
      <checkbox
        testId={`todo-toggle-${todo.id}`}
        value={todo.completed}
        disabled={blocked}
        onChange={() => {
          void onToggle();
        }}
        className="grow basis-0 min-w-0 py-3.5 bg-transparent"
      >
        <row className="items-center gap-3.5">
          <Icon name={todo.completed ? 'success' : 'note'} className={todo.completed ? 'w-6 h-6 text-accent' : 'w-6 h-6 text-faint'} />
          <text testId={`todo-label-${todo.id}`} className={todo.completed ? 'grow basis-0 min-w-0 text-base line-through text-muted' : 'grow basis-0 min-w-0 text-base'}>
            {todo.text}
          </text>
        </row>
      </checkbox>
      <button
        testId={`todo-delete-${todo.id}`}
        accessibilityLabel={`Delete ${todo.text}`}
        disabled={blocked}
        onClick={() => {
          void onDelete();
        }}
        className="w-11 h-11 p-3 rounded-full bg-transparent active:bg-danger-soft disabled:opacity-40"
      >
        <Icon name="close" className="w-5 h-5 text-danger" />
      </button>
    </row>
  );
}

export function render({ data, actions, env }: View<TodoActions, TodoPageData>) {
  const draft = useLocal('draft', '');
  const busy = useLocal('busy', false);
  const failure = useLocal('error', '');
  const blocked = busy.value || data.loading || !!data.loadError;
  const { todos, currentFilter: filter } = data;
  const completed = todos.filter(todo => todo.completed).length;
  const active = todos.length - completed;
  const visible = todos.filter(todo => filter === 'all' || (filter === 'completed' ? todo.completed : !todo.completed));
  const wide = env.viewport.width > 768;
  const run = async (action: () => Promise<unknown>) => {
    if (blocked) return;
    busy.set(true);
    failure.set('');
    try {
      await action();
    } catch {
      failure.set('Could not save this change. Please try again.');
    } finally {
      busy.set(false);
    }
  };
  const add = () => {
    const text = draft.value.trim();
    if (text)
      void run(async () => {
        await actions.addTodo({ text });
        draft.set('');
      });
  };
  return (
    <page testId="todo-page" className="bg-canvas text-text items-center px-3 py-6 gap-4">
      <text className="text-7xl font-thin tracking-tighter text-accent">todos</text>
      <column
        testId="todo-panel"
        className="w-full max-w-xl grow basis-0 min-h-0 rounded-2xl bg-surface/90 backdrop-blur-xl shadow-lg border border-line"
      >
        <row className="items-center">
          {todos.length > 0 ? (
            <button
              testId="todo-toggle-all"
              accessibilityLabel={active ? 'Mark all as complete' : 'Mark all as active'}
              disabled={blocked}
              onClick={() => run(() => actions.toggleAll())}
              className="w-12 h-12 p-3 bg-transparent"
            >
              <Icon name="expand" className={active ? 'w-6 h-6 text-faint' : 'w-6 h-6 text-accent'} />
            </button>
          ) : (
            <spacer className="w-12" />
          )}
          <input
            testId="todo-input"
            accessibilityLabel="New task"
            bind={draft}
            disabled={blocked}
            placeholder="What needs to be done?"
            onSubmit={add}
            className="grow basis-0 min-w-0 min-h-14 py-4 pr-4 bg-transparent text-base placeholder:text-muted focus:bg-surface-sunken/40"
          />
        </row>
        {data.loading && (
          <text testId="todo-loading" className="p-4 text-sm text-muted">
            Loading saved tasks...
          </text>
        )}
        {(data.loadError || failure.value) && (
          <column className="px-4 pb-3 gap-2">
            <ErrorState testId="todo-load-error" error={data.loadError || null} onRetry={() => actions.reload()} />
            <ErrorState testId="todo-error" error={failure.value || null} />
          </column>
        )}
        {!todos.length && !data.loading && !data.loadError ? (
          <EmptyState testId="todo-empty" title="No tasks yet" message="Add a task above to get started" />
        ) : (
          <List
            testId="todo-list"
            className="grow basis-0 min-h-0"
            items={visible}
            getKey={todo => todo.id}
            renderItem={todo => (
              <TodoRow
                todo={todo}
                blocked={blocked}
                onToggle={() => run(() => actions.toggleTodo({ id: todo.id }))}
                onDelete={() => run(() => actions.deleteTodo({ id: todo.id }))}
              />
            )}
          />
        )}
        {todos.length > 0 && (
          <view
            className={
              wide
                ? 'flex-row items-center justify-between gap-3 px-5 py-4 border-t border-line'
                : 'items-stretch gap-2.5 px-4 py-3.5 border-t border-line'
            }
          >
            <text testId="todo-count" className="text-sm text-muted">{`${active} ${active === 1 ? 'item' : 'items'} left`}</text>
            <Segmented
              testId="todo-filter"
              value={filter}
              options={filters}
              disabled={blocked}
              onChange={(value: TodoFilter) => run(() => actions.setFilter({ filter: value }))}
            />
            {completed > 0 && (
              <Button variant="ghost" testId="todo-clear-completed" disabled={blocked} onClick={() => run(() => actions.clearCompleted())}>
                Clear completed
              </Button>
            )}
          </view>
        )}
      </column>
    </page>
  );
}
