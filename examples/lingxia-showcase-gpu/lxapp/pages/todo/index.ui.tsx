import { PageContent, contentBounds } from '../../shared/components/page';
import { useLocal, type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Icon } from '../../shared/components/icon';

import type { TodoActions, TodoPageData } from '../../shared/todo-types';

export function render({ data, actions, env }: View<TodoActions, TodoPageData>) {
  const draft = useLocal('draft', '');
  const busy = useLocal('busy', false);
  const error = useLocal('error', '');
  const blocked = busy.value || !!data.loading || !!data.loadError;
  const todos = data.todos;
  const filter = data.currentFilter;
  const completed = todos.filter(todo => todo.completed).length;
  const visible = todos.filter(todo => filter === 'all' || (filter === 'completed' ? todo.completed : !todo.completed));
  const { width, height } = contentBounds(env);
  const active = todos.length - completed;
  const desktop = width > 768;
  const run = async (action: () => Promise<unknown>) => {
    if (blocked) return;
    busy.set(true);
    error.set('');
    try { await action(); }
    catch { error.set('Could not save this change. Please try again.'); }
    finally { busy.set(false); }
  };
  const add = () => {
    const text = draft.value.trim();
    if (!text) return;
    return run(async () => {
      await actions.addTodo({ text });
      draft.set('');
    });
  };

  return <PageContent testId="todo-page" className="bg-linear-to-b from-[#fffafa] via-white to-[#fffafa] dark:from-[#1c1a1c] dark:via-[#17171a] dark:to-[#1c1a1c] text-[#1d1d1f] dark:text-[#f2f2f7]">
      <stack style={{ width, height }}>
        <column className="items-center" style={{ width, paddingTop: desktop ? 30 : height * (width > 480 ? 0.12 : 0.1) }}>
          <text className="text-[110px] min-[481px]:text-[160px] min-[769px]:text-[100px] tracking-[-0.05em] font-thin text-[#af2f2f]/18 dark:text-[#ff7878]/14">todos</text>
        </column>
        <scroll testId="todo-list" style={{ width, height }}>
          <column className="items-center justify-center px-3 py-6 min-[481px]:px-4 min-[481px]:py-8 min-[769px]:py-10" style={{ minHeight: height }}>
            <column testId="todo-panel" className="backdrop-blur-[20px] backdrop-saturate-180 w-full max-w-[550px] rounded-xl min-[481px]:rounded-[14px] min-[769px]:rounded-2xl bg-white/92 dark:bg-[#1f1f22]/92 shadow-[0_0_0_0.5px_#0000000a,0_2px_4px_#0000000a,0_8px_24px_#00000014] dark:shadow-[0_0_0_0.5px_#ffffff0f,0_2px_4px_#00000066,0_8px_24px_#00000080]">
              <row className="items-center">
                {todos.length > 0 ? <button testId="todo-toggle-all" accessibilityLabel={active ? 'Mark all as complete' : 'Mark all as active'} enabled={!blocked} onClick={() => run(() => actions.toggleAll())} className={active ? "w-12 min-[481px]:w-[52px] min-[769px]:w-[60px] h-12 p-3 bg-transparent text-black/15 dark:text-white/30" : "w-12 min-[481px]:w-[52px] min-[769px]:w-[60px] h-12 p-3 bg-transparent text-[#007aff] dark:text-[#0a84ff]"}>
                  <vector className="w-6 h-6" paths={[{ d: 'M5 9L12 16L19 9', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5 }]} />
                </button> : <spacer className="w-12 min-[481px]:w-[52px] min-[769px]:w-[60px]" />}
                <input accessibilityLabel="New task" testId="todo-input" bind={draft} enabled={!blocked} placeholder="What needs to be done?" onSubmit={add} className="focus:bg-black/1 dark:focus:bg-white/3 placeholder:text-[#3c3c43]/30 dark:placeholder:text-[#ebebf5]/30 grow basis-0 min-w-0 min-h-[54px] py-4 min-[481px]:py-[18px] min-[769px]:py-5 pl-0 pr-4 min-[481px]:pr-[18px] min-[769px]:pr-5 bg-transparent text-[15px] min-[481px]:text-base min-[769px]:text-[17px] leading-[1.47]" />
              </row>
              {data.loading && <text testId="todo-loading" className="p-4 text-sm text-[#3c3c43]/60 dark:text-[#ebebf5]/60">Loading saved tasks...</text>}
              {!!data.loadError && <column className="p-4 gap-2">
                <text testId="todo-load-error" className="text-sm text-red-600 dark:text-red-400">{data.loadError}</text>
                <button testId="todo-retry" onClick={() => actions.reload()} className="min-h-11 bg-transparent text-[#007aff] dark:text-[#0a84ff]">Retry</button>
              </column>}
              {error.value ? <text testId="todo-error" className="p-4 text-sm text-red-600 dark:text-red-400">{error.value}</text> : null}
              {!todos.length && <spacer className="h-px bg-black/6 dark:bg-white/10" />}
              {!todos.length && !data.loading && !data.loadError && <column className="items-center py-12 px-5 min-[769px]:py-[60px] min-[769px]:px-6 text-center">
                <column className="pb-4 min-[769px]:pb-5"><Icon name="note" testId="todo-empty-icon" className="w-14 h-14 min-[769px]:w-16 min-[769px]:h-16 text-[#3c3c43]/30 dark:text-[#ebebf5]/30" /></column>
                <text testId="todo-empty" className="text-lg min-[769px]:text-xl font-medium tracking-[-0.02em] pb-2 text-[#3c3c43]/80 dark:text-[#ebebf5]/85">No tasks yet</text>
                <text className="text-sm min-[769px]:text-[15px] text-[#3c3c43]/60 dark:text-[#ebebf5]/60 opacity-60">Add a task above to get started</text>
              </column>}
              {visible.map(todo => <column key={todo.id} testId={`todo-item-${todo.id}`}>
                <spacer className="h-px bg-black/4 dark:bg-white/7" />
                <row className="items-center min-h-[50px] px-3 gap-2">
                  <checkbox testId={`todo-toggle-${todo.id}`} accessibilityLabel={todo.text} value={todo.completed} enabled={!blocked} onChange={() => run(() => actions.toggleTodo({ id: todo.id }))} className="grow basis-0 min-w-0 px-0 min-h-[50px] min-[481px]:min-h-[54px] min-[769px]:min-h-[58px] py-3.5 min-[481px]:py-4 min-[769px]:py-[18px]">
                    <row className="items-center gap-3.5 min-[481px]:gap-[18px] min-[769px]:gap-[22px]">
                      <vector className={todo.completed ? 'w-6 h-6 shrink-0 text-[#007aff] dark:text-[#0a84ff]' : 'w-6 h-6 shrink-0 text-black/10 dark:text-white/28'} paths={todo.completed ? [
                        { d: 'M22 12A10 10 0 1 1 2 12A10 10 0 1 1 22 12Z', fill: 'currentColor' },
                        { d: 'M9.5 16.5l-3.5-3.5 1.4-1.4 2.1 2.1 5.1-5.1 1.4 1.4z', fill: '#ffffff' },
                      ] : [{ d: 'M22 12A10 10 0 1 1 2 12A10 10 0 1 1 22 12Z', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5 }]} />
                      <text className={todo.completed ? 'grow basis-0 min-w-0 line-through text-[15px] min-[481px]:text-base min-[769px]:text-[17px] leading-[1.4] min-[481px]:leading-[1.43] min-[769px]:leading-[1.47] text-[#3c3c43]/30 dark:text-[#ebebf5]/30' : 'grow basis-0 min-w-0 text-[15px] min-[481px]:text-base min-[769px]:text-[17px] leading-[1.4] min-[481px]:leading-[1.43] min-[769px]:leading-[1.47]'}>{todo.text}</text>
                    </row>
                  </checkbox>
                  <button testId={`todo-delete-${todo.id}`} accessibilityLabel={`Delete ${todo.text}`} enabled={!blocked} onClick={() => run(() => actions.deleteTodo({ id: todo.id }))} className="w-11 h-11 p-3 rounded-full bg-transparent opacity-55 text-[#ff3b30]/80 dark:text-[#ff453a]/85">
                    <vector className="w-5 h-5" paths={[{ d: 'M6 6L18 18M18 6L6 18', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5 }]} />
                  </button>
                </row>
              </column>)}
              {todos.length > 0 && <column>
                <spacer className="h-px bg-black/6 dark:bg-white/10" />
                <view className="flex flex-col min-[769px]:flex-row items-center justify-between flex-wrap gap-2.5 min-[769px]:gap-3 px-4 py-3.5 min-[769px]:px-5 min-[769px]:py-4">
                  <text testId="todo-count" className="text-sm text-[#3c3c43]/60 dark:text-[#ebebf5]/60" style={{ minWidth: desktop ? 120 : 0 }}>{`${active} ${active === 1 ? 'item' : 'items'} left`}</text>
                  <row testId="todo-filters" className="shrink-0 flex-wrap justify-between p-1 gap-1 rounded-lg bg-black/2 dark:bg-white/5" style={desktop ? {} : { width: '100%' }}>
                    {(['all', 'active', 'completed'] as const).map(value => <button key={value} testId={`todo-filter-${value}`} enabled={!blocked} onClick={() => run(() => actions.setFilter({ filter: value }))} className={filter === value ? 'shrink-0 min-w-16 min-[360px]:min-w-[80px] min-[769px]:min-w-[70px] min-h-11 px-2 min-[360px]:px-5 py-2 min-[769px]:px-4 min-[769px]:py-1.5 rounded-md text-[13px] min-[360px]:text-sm min-[769px]:text-[13px] font-medium tracking-[-0.01em] bg-white dark:bg-white/12 shadow-[0_0_0_0.5px_#0000000a,0_1px_2px_#00000014] dark:shadow-[0_0_0_0.5px_#ffffff14,0_1px_2px_#00000066] text-[#007aff] dark:text-[#0a84ff]' : 'shrink-0 min-w-16 min-[360px]:min-w-[80px] min-[769px]:min-w-[70px] min-h-11 px-2 min-[360px]:px-5 py-2 min-[769px]:px-4 min-[769px]:py-1.5 rounded-md text-[13px] min-[360px]:text-sm min-[769px]:text-[13px] font-medium tracking-[-0.01em] bg-transparent text-[#3c3c43]/60 dark:text-[#ebebf5]/60'}>{value === 'all' ? 'All' : value === 'active' ? 'Active' : 'Completed'}</button>)}
                  </row>
                  {completed > 0 && <button testId="todo-clear-completed" enabled={!blocked} onClick={() => run(() => actions.clearCompleted())} style={desktop ? {} : { width: '100%' }} className="min-h-11 py-2.5 min-[769px]:py-1.5 min-[769px]:px-3 text-sm min-[769px]:text-[13px] font-medium tracking-[-0.01em] rounded-md bg-transparent text-[#3c3c43]/60 dark:text-[#ebebf5]/60">Clear completed</button>}
                </view>
              </column>}
            </column>
          </column>
        </scroll>
      </stack>
  </PageContent>;
}
