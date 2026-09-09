export interface Todo {
  id: string;
  text: string;
  completed: boolean;
}

// Common todo utilities

// The Rong Logic profile has no `crypto`, so there is no `randomUUID` to prefer.
export function generateTodoId(): string {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).slice(2, 8);
  return `todo_${timestamp}_${random}`;
}

export function validateTodoText(text: unknown): text is string {
  return text != null && typeof text === "string" && text.trim().length > 0;
}

export function getCurrentTimestamp(): string {
  return new Date().toISOString();
}
