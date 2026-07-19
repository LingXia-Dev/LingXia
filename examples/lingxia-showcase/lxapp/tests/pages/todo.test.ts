import { expect, test } from '@rongjs/test';
import type { LxAppDriver } from 'lingxia-types';

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

async function waitForInputValue(app: LxAppDriver, css: string, value: string): Promise<void> {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const input = await app.page.query({ page: 'todo', css, full: true });
    if (input.exists && input.value === value) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for todo input value: ${value}`);
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

test('adds and removes a todo through the rendered page', async () => {
  const app = lx.automation().lxapp();
  await app.nav.relaunch({ page: 'todo' });
  await app.page.waitFor({ page: 'todo', css: '[data-testid="todo-page"]' });

  const text = `automation todo ${Date.now()}`;
  const input = '[data-testid="todo-input"]';
  try {
    await app.page.fill({ page: 'todo', css: input, text });
    await waitForInputValue(app, input, text);
    await app.page.press({ page: 'todo', css: input, key: 'Enter' });
    await app.page.waitFor({ page: 'todo', css: '[data-testid="todo-item"]' });

    const index = await waitForTodo(app, text, true);
    expect(index >= 0).toBeTruthy();
    await waitForStoredTodo(app, text, true);

    await app.page.waitFor({
      page: 'todo',
      css: '[data-testid="todo-delete"]',
      state: 'visible',
    });
    await app.page.click({
      page: 'todo',
      css: '[data-testid="todo-delete"]',
      index,
    });
    expect(await waitForTodo(app, text, false)).toBe(-1);
    await waitForStoredTodo(app, text, false);
  } catch (error) {
    const screenshot = await app.page.screenshot({ page: 'todo' });
    await test.attach?.('todo-page-failure.png', {
      mimeType: 'image/png',
      base64: screenshot.base64,
    });
    throw error;
  } finally {
    await cleanupStoredTodo(app, text);
  }
});
