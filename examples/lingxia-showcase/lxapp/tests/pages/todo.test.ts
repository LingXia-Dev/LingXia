import { expect, test } from '@rongjs/test';
import type { LxAppDriver } from 'lingxia-types';
import { waitForElementAttribute } from '../helpers/page.js';

async function waitForTodo(app: LxAppDriver, text: string, present: boolean): Promise<number> {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const labels = await app.page.query({
      page: 'todo',
      css: '[data-testid="todo-label"]',
      all: true,
      full: true,
    });
    const index = labels.items.findIndex((label) => label.text === text);
    if ((index >= 0) === present) return index;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for todo to be ${present ? 'present' : 'removed'}: ${text}`);
}

async function waitForStoredTodo(
  app: LxAppDriver,
  text: string,
  present: boolean,
): Promise<void> {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const stored = await app.eval({
      script: `
        const todos = await lx.getStorage().get('todo:todos');
        return Array.isArray(todos) && todos.some((todo) => todo.text === ${JSON.stringify(text)});
      `,
    });
    if (stored === present) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for persisted todo to be ${present ? 'present' : 'removed'}: ${text}`);
}

async function waitForStoredCompleted(
  app: LxAppDriver,
  text: string,
  completed: boolean,
): Promise<void> {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const stored = await app.eval({
      script: `
        const todos = await lx.getStorage().get('todo:todos');
        const todo = Array.isArray(todos) && todos.find((item) => item.text === ${JSON.stringify(text)});
        return todo ? todo.completed === ${completed} : false;
      `,
    });
    if (stored === true) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for persisted todo completion=${completed}: ${text}`);
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

test('adds and removes a todo through the rendered page', async () => {
  const app = lx.automation().lxapp();
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
