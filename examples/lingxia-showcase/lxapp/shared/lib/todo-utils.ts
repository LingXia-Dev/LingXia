// Common todo utilities
import { uniqueId } from "lodash-es";

export function generateTodoId(): string {
  if (
    typeof crypto !== "undefined" &&
    typeof crypto.randomUUID === "function"
  ) {
    return `todo_${crypto.randomUUID()}`;
  }
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).slice(2, 8);
  return `todo_${timestamp}_${random}`;
}

export function validateTodoText(text: unknown): boolean {
  return text != null && typeof text === "string" && text.trim().length > 0;
}

export function getCurrentTimestamp(): string {
  return new Date().toISOString();
}
