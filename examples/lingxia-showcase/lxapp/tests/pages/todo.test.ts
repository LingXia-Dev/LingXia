import { spec, type LogicPage, type TestApp } from '@lingxia/test';
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
  const view = app.view;

  await t.expect(view.testId('todo-page', { page: 'todo' })).toBeVisible();

  const text = `automation todo ${Date.now()}`;
  pendingTodo = text;
  t.defer(async () => {
    await cleanupStoredTodo(app, text);
    pendingTodo = undefined;
  });
  try {
    const input = view.testId('todo-input', { page: 'todo' });
    await input.fill(text);
    // `fill` writes through the input's value setter and dispatches
    // input/change, so the framework-controlled value follows it.
    await t.expect(input).toHaveAttribute('data-controlled-value', text);
    await input.press('Enter');

    const label = view.testId('todo-label', { page: 'todo' }).filter({ hasText: text });
    await t.expect(label).toBeVisible();
    await t.expect(() => storedTodo(app, text), { timeout: 30_000 }).toEqual({ completed: false });
    const { todos } = await app.logic.data<{ todos: Todo[] }>({ page: 'todo' });
    t.expect(todos.some((todo) => todo.text === text)).toBe(true);

    await label.click();
    await t.expect(() => storedTodo(app, text), { timeout: 30_000 }).toEqual({ completed: true });

    await view.testId('todo-filter-completed', { page: 'todo' }).click();
    await t.expect(label).toBeVisible();
    await view.testId('todo-filter-active', { page: 'todo' }).click();
    await t.expect(label).toHaveCount(0);
    await view.testId('todo-filter-all', { page: 'todo' }).click();

    await label.click();
    await t.expect(() => storedTodo(app, text), { timeout: 30_000 }).toEqual({ completed: false });

    const screenshot = await view.screenshot({ page: 'todo' });
    await attachShot(t, 'todo-page.png', {
      mimeType: 'image/png',
      base64: screenshot.base64,
    });

    // The delete button sits beside the label: find the row by its text.
    const row = await view.eval(({ document }, wanted) =>
      Array.from(document.querySelectorAll('[data-testid="todo-label"]'))
        .findIndex((element) => element.textContent?.trim() === wanted), text);
    await view.testId('todo-delete', { page: 'todo', index: row }).click();
    await t.expect(label).toHaveCount(0);
    await t.expect(() => storedTodo(app, text), { timeout: 30_000 }).toBe(null);
  } catch (error) {
    try {
      const screenshot = await view.screenshot({ page: 'todo' });
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
  await t.expect(app.view.testId('todo-page', { page: 'todo' })).toBeVisible();

  const text = `logic todo ${Date.now()}`;
  pendingTodo = text;
  t.defer(async () => {
    await cleanupStoredTodo(app, text);
    pendingTodo = undefined;
  });

  await app.logic.call<TodoPage, 'addTodo'>('addTodo', { text });
  const label = app.view.testId('todo-label', { page: 'todo' }).filter({ hasText: text });
  await t.expect(label).toBeVisible();

  const added = await t.waitFor(
    () => app.logic.data<{ todos: Todo[] }>({ page: 'todo' }),
    { until: (data) => data.todos.some((todo) => todo.text === text), timeout: 10_000 },
  );
  const id = added.todos.find((todo) => todo.text === text)!.id;
  await app.logic.call<TodoPage, 'deleteTodo'>('deleteTodo', { id });
  await t.expect(label).toHaveCount(0);
  await t.expect(() => storedTodo(app, text), { timeout: 30_000 }).toBe(null);
});
