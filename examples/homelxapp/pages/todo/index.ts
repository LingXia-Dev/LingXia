import {
  generateTodoId,
  validateTodoText,
  getCurrentTimestamp,
} from "../../shared/lib/todo-utils";

function getInitialData() {
  return {
    todos: [],
    currentFilter: "all",
    lastUpdated: getCurrentTimestamp(),
  };
}

Page({
  data: getInitialData(),

  // Storage keys
  STORAGE_KEYS: {
    TODOS: "todo:todos",
    FILTER: "todo:filter",
    LAST_UPDATED: "todo:lastUpdated",
  },

  // Page lifecycle
  onLoad: async function () {
    console.log("[Todo] Page loaded, initial todos:", this.data.todos.length);
    await this._loadFromStorage();
  },

  onReady: function () {
    console.log("[Todo] Page ready");
  },

  onShow: function () {
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

  _ensureStorage: function () {
    if (!this._storage) {
      this._storage = lx.getStorage();
    }
    return this._storage;
  },

  _loadFromStorage: async function () {
    try {
      const storage = this._ensureStorage();
      const [storedTodos, storedFilter, storedLastUpdated] = await Promise.all([
        storage.get(this.STORAGE_KEYS.TODOS),
        storage.get(this.STORAGE_KEYS.FILTER),
        storage.get(this.STORAGE_KEYS.LAST_UPDATED),
      ]);

      if (storedTodos && Array.isArray(storedTodos) && storedTodos.length > 0) {
        console.log("[Todo] Loaded stored todos:", storedTodos.length);
        this.setData({
          todos: storedTodos,
          currentFilter: storedFilter || "all",
          lastUpdated: storedLastUpdated || getCurrentTimestamp(),
        });
      } else {
        console.log("[Todo] No stored todos found");
      }
    } catch (error) {
      console.error("[Todo] _loadFromStorage error:", error);
    }
  },

  _saveToStorage: async function (overrides = {}) {
    try {
      const storage = this._ensureStorage();
      const todos = overrides.todos ?? this.data.todos;
      const filter = overrides.filter ?? this.data.currentFilter;
      const lastUpdated = overrides.lastUpdated ?? this.data.lastUpdated;
      await Promise.all([
        storage.set(this.STORAGE_KEYS.TODOS, todos),
        storage.set(this.STORAGE_KEYS.FILTER, filter),
        storage.set(this.STORAGE_KEYS.LAST_UPDATED, lastUpdated),
      ]);
      console.log("[Todo] saveToStorage: Successfully saved to storage");
    } catch (error) {
      console.error("[Todo] saveToStorage: Error saving to storage:", error);
    }
  },

  _clearStorage: async function () {
    try {
      const storage = this._ensureStorage();
      await Promise.all([
        storage.delete(this.STORAGE_KEYS.TODOS),
        storage.delete(this.STORAGE_KEYS.FILTER),
        storage.delete(this.STORAGE_KEYS.LAST_UPDATED),
      ]);
      console.log("[Todo] clearStorage: Successfully cleared storage");
    } catch (error) {
      console.error("[Todo] clearStorage: Error clearing storage:", error);
    }
  },

  // Todo core functionality
  addTodo: async function (params = {}) {
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

    const newTodos = [...this.data.todos, newTodo];
    const lastUpdated = getCurrentTimestamp();
    console.log("[Todo] addTodo: Adding new todo:", newTodo);

    await new Promise((resolve) =>
      this.setData({ todos: newTodos, lastUpdated }, resolve),
    );
    await this._saveToStorage({ todos: newTodos, lastUpdated });

    console.log(
      "[Todo] addTodo: Todo added successfully, total todos:",
      newTodos.length,
    );
  },

  toggleTodo: async function (params = {}) {
    const { id } = params;
    if (!id) {
      console.log("[Todo] toggleTodo: No ID provided, skipping");
      return;
    }

    const targetTodo = this.data.todos.find((todo) => todo.id === id);
    console.log("[Todo] toggleTodo: Target todo:", targetTodo);

    const updatedTodos = this.data.todos.map((todo) =>
      todo.id === id ? { ...todo, completed: !todo.completed } : todo,
    );
    const lastUpdated = new Date().toISOString();

    await new Promise((resolve) =>
      this.setData({ todos: updatedTodos, lastUpdated }, resolve),
    );
    await this._saveToStorage({ todos: updatedTodos, lastUpdated });

    console.log("[Todo] toggleTodo: Todo toggled successfully");
  },

  deleteTodo: async function (params = {}) {
    const { id } = params;
    if (!id) {
      console.log("[Todo] deleteTodo: No ID provided, skipping");
      return;
    }

    const targetTodo = this.data.todos.find((todo) => todo.id === id);
    console.log("[Todo] deleteTodo: Deleting todo:", targetTodo);

    const updatedTodos = this.data.todos.filter((todo) => todo.id !== id);
    const lastUpdated = new Date().toISOString();

    await new Promise((resolve) =>
      this.setData({ todos: updatedTodos, lastUpdated }, resolve),
    );
    await this._saveToStorage({ todos: updatedTodos, lastUpdated });

    console.log(
      "[Todo] deleteTodo: Todo deleted successfully, remaining todos:",
      updatedTodos.length,
    );
  },

  clearCompleted: async function () {
    const completedCount = this.data.todos.filter(
      (todo) => todo.completed,
    ).length;
    console.log(
      "[Todo] clearCompleted: Clearing",
      completedCount,
      "completed todos",
    );

    const updatedTodos = this.data.todos.filter((todo) => !todo.completed);
    const lastUpdated = new Date().toISOString();

    await new Promise((resolve) =>
      this.setData({ todos: updatedTodos, lastUpdated }, resolve),
    );
    await this._saveToStorage({ todos: updatedTodos, lastUpdated });

    console.log(
      "[Todo] clearCompleted: Completed todos cleared, remaining todos:",
      updatedTodos.length,
    );
  },

  setFilter: async function (params = {}) {
    console.log("[Todo] setFilter called with params:", params);
    const { filter } = params;
    if (!filter || !["all", "active", "completed"].includes(filter)) {
      console.log("[Todo] setFilter: Invalid filter value, skipping");
      return;
    }

    console.log("[Todo] setFilter: Setting filter to:", filter);
    const lastUpdated = new Date().toISOString();
    await new Promise((resolve) =>
      this.setData({ currentFilter: filter, lastUpdated }, resolve),
    );
    await this._saveToStorage({ filter, lastUpdated });

    console.log("[Todo] setFilter: Filter set successfully");
  },

  // Debug method (can be removed in production)
  _getStorageInfo: async function () {
    try {
      const storage = this._ensureStorage();
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
});
