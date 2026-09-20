import type { TestApp } from '@lingxia/test';
import { expect, spec } from '@lingxia/test';
import { waitForElementAttribute } from '../helpers/page.js';
import { attachShot, bindFixture, eventually } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

async function waitForTodo(app: TestApp, text: string, present: boolean): Promise<number> {
  return eventually(
    async () => {
      const labels = await app.page.query({
      page: 'todo',
      css: '[data-testid="todo-label"]',
      all: true,
      full: true,
      });
      return labels.items.findIndex((label) => label.text === text);
    },
    (index) => (index >= 0) === present,
    { describe: `todo to be ${present ? 'present' : 'removed'}: ${text}`, timeoutMs: 30_000 });
}

async function waitForStoredTodo(
  app: TestApp,
  text: string,
  present: boolean,
): Promise<void> {
  await eventually(
    () => app.eval({
      script: `
        const todos = await lx.getStorage().get('todo:todos');
        return Array.isArray(todos) && todos.some((todo) => todo.text === ${JSON.stringify(text)});
      `,
    }),
    (stored) => stored === present,
    { describe: `persisted todo to be ${present ? 'present' : 'removed'}: ${text}`, timeoutMs: 30_000 });
}

async function waitForStoredCompleted(
  app: TestApp,
  text: string,
  completed: boolean,
): Promise<void> {
  await eventually(
    () => app.eval({
      script: `
        const todos = await lx.getStorage().get('todo:todos');
        const todo = Array.isArray(todos) && todos.find((item) => item.text === ${JSON.stringify(text)});
        return todo ? todo.completed === ${completed} : false;
      `,
    }),
    (stored) => stored === true,
    { describe: `persisted todo completion=${completed}: ${text}`, timeoutMs: 30_000 });
}

async function cleanupStoredTodo(app: TestApp, text: string): Promise<void> {
  await app.eval({
    script: `
      const storage = lx.getStorage();
      const todos = await storage.get('todo:todos');
      if (Array.isArray(todos)) {
        await storage.set('todo:todos', todos.filter((todo) => todo.text !== ${JSON.stringify(text)}));
      }
    `,
  });
}

async function clickTodoToggle(app: TestApp, index: number): Promise<void> {
  await app.page.testId("todo-label", { page: 'todo', index: index }).click();
}

let pendingTodo: string | undefined;

spec.reset(async t => {
  if (pendingTodo) await cleanupStoredTodo(t.app, pendingTodo);
  await t.app.nav.relaunch({ page: 'todo' });
});

spec("persist todo edits made through the rendered page", { id: "TODO-001", covers: ['lx.getStorage', 'Storage.get', 'Storage.set'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "TODO-001");

  await app.page.waitFor({ page: 'todo', css: '[data-testid="todo-page"]' });

  const text = `automation todo ${Date.now()}`;
  pendingTodo = text;
  t.defer(async () => {
    await cleanupStoredTodo(app, text);
    pendingTodo = undefined;
  });
  const input = '[data-testid="todo-input"]';
  try {
    await app.page.css(input, { page: 'todo' }).fill(text);
    await waitForElementAttribute(app, 'todo', input, 'data-controlled-value', text);
    await app.page.css(input, { page: 'todo' }).press('Enter');
    await app.page.waitFor({ page: 'todo', css: '[data-testid="todo-item"]' });

    const index = await waitForTodo(app, text, true);
    expect(index).toBeGreaterThanOrEqual(0);
    await waitForStoredTodo(app, text, true);

    await clickTodoToggle(app, index);
    await waitForStoredCompleted(app, text, true);

    await app.page.testId("todo-filter-completed", { page: 'todo' }).click();
    expect(await waitForTodo(app, text, true)).toBeGreaterThanOrEqual(0);
    await app.page.testId("todo-filter-active", { page: 'todo' }).click();
    expect(await waitForTodo(app, text, false)).toBe(-1);
    await app.page.testId("todo-filter-all", { page: 'todo' }).click();

    const completedIndex = await waitForTodo(app, text, true);
    await clickTodoToggle(app, completedIndex);
    await waitForStoredCompleted(app, text, false);

    const screenshot = await app.page.screenshot({ page: 'todo' });
    await attachShot(t, 'todo-page.png', {
      mimeType: 'image/png',
      base64: screenshot.base64,
    });

    const activeIndex = await waitForTodo(app, text, true);
    await app.page.testId("todo-delete", { page: 'todo', index: activeIndex }).click();
    expect(await waitForTodo(app, text, false)).toBe(-1);
    await waitForStoredTodo(app, text, false);
  } catch (error) {
    try {
      const screenshot = await app.page.screenshot({ page: 'todo' });
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
