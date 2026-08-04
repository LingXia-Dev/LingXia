import { expect, test } from '@rongjs/test';
import type { LxAppDriver } from 'lingxia-types/automation';
import { waitForElementAttribute } from '../helpers/page.js';
import { contract, eventually } from '../support/contract.js';

async function waitForTodo(app: LxAppDriver, text: string, present: boolean): Promise<number> {
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
    { describe: `todo to be ${present ? 'present' : 'removed'}: ${text}`, timeoutMs: 30_000 },
  );
}

async function waitForStoredTodo(
  app: LxAppDriver,
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
    { describe: `persisted todo to be ${present ? 'present' : 'removed'}: ${text}`, timeoutMs: 30_000 },
  );
}

async function waitForStoredCompleted(
  app: LxAppDriver,
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
    { describe: `persisted todo completion=${completed}: ${text}`, timeoutMs: 30_000 },
  );
}

async function cleanupStoredTodo(app: LxAppDriver, text: string): Promise<void> {
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

async function clickTodoToggle(app: LxAppDriver, index: number): Promise<void> {
  await app.page.click({
    page: 'todo',
    css: '[data-testid="todo-label"]',
    index,
  });
}

contract({
  id: 'TODO-001',
  title: 'persist todo edits made through the rendered page',
  covers: ['lx.getStorage', 'Storage.get', 'Storage.set'],
  layer: 'logic',
  levels: ['semantic', 'boundary', 'lifecycle'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app }) => {
  await app.nav.relaunch({ page: 'todo' });
  await app.page.waitFor({ page: 'todo', css: '[data-testid="todo-page"]' });

  const text = `automation todo ${Date.now()}`;
  const input = '[data-testid="todo-input"]';
  try {
    await app.page.fill({ page: 'todo', css: input, text });
    await waitForElementAttribute(app, 'todo', input, 'data-controlled-value', text);
    await app.page.press({ page: 'todo', css: input, key: 'Enter' });
    await app.page.waitFor({ page: 'todo', css: '[data-testid="todo-item"]' });

    const index = await waitForTodo(app, text, true);
    expect(index >= 0).toBeTruthy();
    await waitForStoredTodo(app, text, true);

    await clickTodoToggle(app, index);
    await waitForStoredCompleted(app, text, true);

    await app.page.click({ page: 'todo', css: '[data-testid="todo-filter-completed"]' });
    expect(await waitForTodo(app, text, true) >= 0).toBeTruthy();
    await app.page.click({ page: 'todo', css: '[data-testid="todo-filter-active"]' });
    expect(await waitForTodo(app, text, false)).toBe(-1);
    await app.page.click({ page: 'todo', css: '[data-testid="todo-filter-all"]' });

    const completedIndex = await waitForTodo(app, text, true);
    await clickTodoToggle(app, completedIndex);
    await waitForStoredCompleted(app, text, false);

    const screenshot = await app.page.screenshot({ page: 'todo' });
    await test.attach?.('todo-page.png', {
      mimeType: 'image/png',
      base64: screenshot.base64,
    });

    const activeIndex = await waitForTodo(app, text, true);
    await app.page.click({
      page: 'todo',
      css: '[data-testid="todo-delete"]',
      index: activeIndex,
    });
    expect(await waitForTodo(app, text, false)).toBe(-1);
    await waitForStoredTodo(app, text, false);
  } catch (error) {
    try {
      const screenshot = await app.page.screenshot({ page: 'todo' });
      await test.attach?.('todo-page-failure.png', {
        mimeType: 'image/png',
        base64: screenshot.base64,
      });
    } catch {
      // Preserve the todo failure when screenshot capture also fails.
    }
    throw error;
  } finally {
    await cleanupStoredTodo(app, text);
  }
});
