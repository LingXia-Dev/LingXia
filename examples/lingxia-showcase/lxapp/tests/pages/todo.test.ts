import { spec, type LogicPage, type TestApp, expect } from '@lingxia/test';
import { attachShot, bindFixture } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

interface Todo {
  id: string;
  text: string;
  completed: boolean;
}

/** The todo page as its Logic declares it, for `t.app.logic.call`/`data`. */
interface TodoPage extends LogicPage<{ todos: Todo[]; currentFilter: string }> {
  addTodo(params: { text?: string }): Promise<void>;
  deleteTodo(params: { id?: string }): Promise<void>;
}

/** The persisted todo with `text`, or `null`. */
function storedTodo(app: TestApp, text: string): Promise<{ completed: boolean } | null> {
  return app.logic.eval(async ({ lx }, wanted) => {
    const todos = (await lx.getStorage().get('todo:todos')) as Todo[] | undefined;
    const todo = Array.isArray(todos) ? todos.find((item) => item.text === wanted) : undefined;
    return todo ? { completed: todo.completed } : null;
  }, text);
}

async function cleanupStoredTodo(app: TestApp, text: string): Promise<void> {
  await app.logic.eval(async ({ lx }, wanted) => {
    const storage = lx.getStorage();
    const todos = (await storage.get('todo:todos')) as Todo[] | undefined;
    if (Array.isArray(todos)) {
      await storage.set('todo:todos', todos.filter((todo) => todo.text !== wanted));
    }
  }, text);
}

let pendingTodo: string | undefined;

spec.reset(async t => {
  if (pendingTodo) await cleanupStoredTodo(t.app, pendingTodo);
  await t.app.nav.relaunch({ page: 'todo' });
});

spec("persist todo edits made through the rendered page", { id: "TODO-001", covers: ['lx.getStorage', 'Storage.get', 'Storage.set'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "TODO-001");
  const todo = app.view.page('todo');

  await expect(todo.testId('todo-page')).toBeVisible();

  const text = `automation todo ${Date.now()}`;
  pendingTodo = text;
  t.defer(async () => {
    await cleanupStoredTodo(app, text);
    pendingTodo = undefined;
  });
  try {
    const input = todo.testId('todo-input');
    await input.fill(text);
    // `fill` writes through the input's value setter and dispatches
    // input/change, so the framework-controlled value follows it.
    await expect(input).toHaveAttribute('data-controlled-value', text);
    await input.press('Enter');

    const label = todo.testId('todo-label').filter({ hasText: text });
    await expect(label).toBeVisible();
    await expect.poll(() => storedTodo(app, text), { timeout: 30_000 }).toEqual({ completed: false });
    const { todos } = await app.logic.data<{ todos: Todo[] }>({ page: 'todo' });
    expect(todos.some((todo) => todo.text === text)).toBe(true);

    await label.click();
    await expect.poll(() => storedTodo(app, text), { timeout: 30_000 }).toEqual({ completed: true });

    await todo.testId('todo-filter-completed').click();
    await expect(label).toBeVisible();
    await todo.testId('todo-filter-active').click();
    await expect(label).toHaveCount(0);
    await todo.testId('todo-filter-all').click();

    await label.click();
    await expect.poll(() => storedTodo(app, text), { timeout: 30_000 }).toEqual({ completed: false });

    const screenshot = await todo.screenshot();
    await attachShot(t, 'todo-page.png', {
      mimeType: 'image/png',
      base64: screenshot.base64,
    });

    // The delete button sits beside the label: find the row by its text.
    const row = await todo.eval(({ document }, wanted) =>
      Array.from(document.querySelectorAll('[data-testid="todo-label"]'))
        .findIndex((element) => element.textContent?.trim() === wanted), text);
    await todo.testId('todo-delete').nth(row).click();
    await expect(label).toHaveCount(0);
    await expect.poll(() => storedTodo(app, text), { timeout: 30_000 }).toBe(null);
  } catch (error) {
    try {
      const screenshot = await todo.screenshot();
      await attachShot(t, 'todo-page-failure.png', {
        mimeType: 'image/png',
        base64: screenshot.base64,
      });
    } catch {
      // Preserve the todo failure when screenshot capture also fails.
    }
    throw error;
  }
});

spec("add and delete a todo through the page's Logic methods", { id: "TODO-002", app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "TODO-002");
  const todo = app.view.page('todo');
  await expect(todo.testId('todo-page')).toBeVisible();

  const text = `logic todo ${Date.now()}`;
  pendingTodo = text;
  t.defer(async () => {
    await cleanupStoredTodo(app, text);
    pendingTodo = undefined;
  });

  await app.logic.call<TodoPage, 'addTodo'>('addTodo', { text });
  const label = todo.testId('todo-label').filter({ hasText: text });
  await expect(label).toBeVisible();

  const added = await t.waitFor(
    () => app.logic.data<{ todos: Todo[] }>({ page: 'todo' }),
    { until: (data) => data.todos.some((todo) => todo.text === text), timeout: 10_000 },
  );
  const id = added.todos.find((todo) => todo.text === text)!.id;
  await app.logic.call<TodoPage, 'deleteTodo'>('deleteTodo', { id });
  await expect(label).toHaveCount(0);
  await expect.poll(() => storedTodo(app, text), { timeout: 30_000 }).toBe(null);
});
