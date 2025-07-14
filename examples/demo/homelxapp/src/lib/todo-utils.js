// Common todo utilities
import { uniqueId } from 'lodash-es';

export function generateTodoId() {
  return uniqueId('todo_');
}

export function validateTodoText(text) {
  return text && typeof text === 'string' && text.trim().length > 0;
}

export function getCurrentTimestamp() {
  return new Date().toISOString();
}