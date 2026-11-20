import type { TypedStorage } from "@lingxia/types";
import {
  generateTodoId,
  validateTodoText,
  getCurrentTimestamp,
  type Todo,
} from "../../shared/lib/todo-utils";
import { definePage } from '@lingxia/types/page';

// Pinning the schema means `get` returns the stored shape instead of `unknown`,
// and a key typo is a compile error.
interface TodoStorage {
  "todo:todos": Todo[];
  "todo:filter": string;
  "todo:lastUpdated": string;
}

function getInitialData() {
  return {
    todos: [] as Todo[],
    currentFilter: "all",
    lastUpdated: getCurrentTimestamp(),
  };
}

export default definePage({
  data: getInitialData(),
  setup(page) {
    // Storage keys
    let STORAGE_KEYS = {
      TODOS: "todo:todos",
      FILTER: "todo:filter",
      LAST_UPDATED: "todo:lastUpdated",
    } as const;
    // Page lifecycle
    let _storage = null as TypedStorage<TodoStorage> | null;
    const helpers = {
      ensureStorage() {
        if (!_storage) {
          _storage = lx.getStorage<TodoStorage>();
        }
        return _storage;
      },
      async loadFromStorage() {
        try {
          const storage = helpers.ensureStorage();
          const todosKey = STORAGE_KEYS.TODOS;
          if (!(await storage.has(todosKey))) {
            console.log("[Todo] No stored todos found");
            return;
          }
          const [storedTodos, storedFilter, storedLastUpdated] = await Promise.all([
            storage.get(todosKey),
            storage.get(STORAGE_KEYS.FILTER),
            storage.get(STORAGE_KEYS.LAST_UPDATED),
          ]);

          if (storedTodos && Array.isArray(storedTodos) && storedTodos.length > 0) {
            console.log("[Todo] Loaded stored todos:", storedTodos.length);
            page.update(d => {
              d.todos = storedTodos;
              d.currentFilter = storedFilter || "all";
              d.lastUpdated = storedLastUpdated || getCurrentTimestamp();
            });
          } else {
            console.log("[Todo] No stored todos found");
          }
        } catch (error) {
          console.error("[Todo] _loadFromStorage error:", error);
        }
      },
      async saveToStorage(overrides: { todos?: Todo[]; filter?: string; lastUpdated?: string } = {}) {
        try {
          const storage = helpers.ensureStorage();
          const todos = overrides.todos ?? page.data.todos;
          const filter = overrides.filter ?? page.data.currentFilter;
          const lastUpdated = overrides.lastUpdated ?? page.data.lastUpdated;
          await Promise.all([
            storage.set(STORAGE_KEYS.TODOS, todos),
            storage.set(STORAGE_KEYS.FILTER, filter),
            storage.set(STORAGE_KEYS.LAST_UPDATED, lastUpdated),
          ]);
          console.log("[Todo] saveToStorage: Successfully saved to storage");
        } catch (error) {
          console.error("[Todo] saveToStorage: Error saving to storage:", error);
        }
      },
      async clearStorage() {
        try {
          const storage = helpers.ensureStorage();
          await Promise.all([
            storage.delete(STORAGE_KEYS.TODOS),
            storage.delete(STORAGE_KEYS.FILTER),
            storage.delete(STORAGE_KEYS.LAST_UPDATED),
          ]);
          console.log("[Todo] clearStorage: Successfully cleared storage");
        } catch (error) {
          console.error("[Todo] clearStorage: Error clearing storage:", error);
        }
      },
      // Debug method (can be removed in production)
      async getStorageInfo() {
        try {
          const storage = helpers.ensureStorage();
          const info = await storage.info();
          console.log("[Todo] Storage info:", info);
          return info;
        } catch (error) {
          console.error(
            "[Todo] getStorageInfo: Error getting storage info:",
            error,
          );
          return null;
        }
      },
    };
    const actions = {
      // Todo core functionality
      async addTodo(params: { text?: string } = {}) {
        const { text } = params;
        if (!validateTodoText(text)) {
          console.log("[Todo] addTodo: Invalid text, skipping");
          return;
        }

        const newTodo = {
          id: generateTodoId(),
          text: text.trim(),
          completed: false,
        };

        const newTodos = [...page.data.todos, newTodo];
        const lastUpdated = getCurrentTimestamp();
        console.log("[Todo] addTodo: Adding new todo:", newTodo);

        page.update(d => {
          d.todos = newTodos;
          d.lastUpdated = lastUpdated;
        });
        await page.flush();
        await helpers.saveToStorage({ todos: newTodos, lastUpdated });

        console.log(
          "[Todo] addTodo: Todo added successfully, total todos:",
          newTodos.length,
        );
      },
      async toggleTodo(params: { id?: string } = {}) {
        const { id } = params;
        if (!id) {
          console.log("[Todo] toggleTodo: No ID provided, skipping");
          return;
        }

        const targetTodo = page.data.todos.find((todo) => todo.id === id);
        console.log("[Todo] toggleTodo: Target todo:", targetTodo);

        const updatedTodos = page.data.todos.map((todo) =>
          todo.id === id ? { ...todo, completed: !todo.completed } : todo,
        );
        const lastUpdated = new Date().toISOString();

        page.update(d => {
          d.todos = updatedTodos;
          d.lastUpdated = lastUpdated;
        });
        await page.flush();
        await helpers.saveToStorage({ todos: updatedTodos, lastUpdated });

        console.log("[Todo] toggleTodo: Todo toggled successfully");
      },
      async deleteTodo(params: { id?: string } = {}) {
        const { id } = params;
        if (!id) {
          console.log("[Todo] deleteTodo: No ID provided, skipping");
          return;
        }

        const targetTodo = page.data.todos.find((todo) => todo.id === id);
        console.log("[Todo] deleteTodo: Deleting todo:", targetTodo);

        const updatedTodos = page.data.todos.filter((todo) => todo.id !== id);
        const lastUpdated = new Date().toISOString();

        page.update(d => {
          d.todos = updatedTodos;
          d.lastUpdated = lastUpdated;
        });
        await page.flush();
        await helpers.saveToStorage({ todos: updatedTodos, lastUpdated });

        console.log(
          "[Todo] deleteTodo: Todo deleted successfully, remaining todos:",
          updatedTodos.length,
        );
      },
      async clearCompleted() {
        const completedCount = page.data.todos.filter(
          (todo) => todo.completed,
        ).length;
        console.log(
          "[Todo] clearCompleted: Clearing",
          completedCount,
          "completed todos",
        );

        const updatedTodos = page.data.todos.filter((todo) => !todo.completed);
        const lastUpdated = new Date().toISOString();

        page.update(d => {
          d.todos = updatedTodos;
          d.lastUpdated = lastUpdated;
        });
        await page.flush();
        await helpers.saveToStorage({ todos: updatedTodos, lastUpdated });

        console.log(
          "[Todo] clearCompleted: Completed todos cleared, remaining todos:",
          updatedTodos.length,
        );
      },
      async setFilter(params: { filter?: string } = {}) {
        console.log("[Todo] setFilter called with params:", params);
        const { filter } = params;
        if (!filter || !["all", "active", "completed"].includes(filter)) {
          console.log("[Todo] setFilter: Invalid filter value, skipping");
          return;
        }

        console.log("[Todo] setFilter: Setting filter to:", filter);
        const lastUpdated = new Date().toISOString();
        page.update(d => {
          d.currentFilter = filter;
          d.lastUpdated = lastUpdated;
        });
        await page.flush();
        await helpers.saveToStorage({ filter, lastUpdated });

        console.log("[Todo] setFilter: Filter set successfully");
      },
    };
    return {
      async onLoad() {
        console.log("[Todo] Page loaded, initial todos:", page.data.todos.length);
        await helpers.loadFromStorage();
      },
      onReady() {
        console.log("[Todo] Page ready");
      },
      onShow() {
        console.log("[Todo] Page shown");
        // Test bundled utilities access - use direct function calls
        if (typeof generateTodoId !== "undefined") {
          const testId = generateTodoId();
          console.log(
            "[Todo] Generated test UUID using bundled utilities:",
            testId,
          );
        } else {
          console.log("[Todo] generateTodoId function not available");
        }
      },
      actions,
    };
  },
});
