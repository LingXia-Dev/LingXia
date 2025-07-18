import { generateTodoId, validateTodoText, getCurrentTimestamp } from '../../src/lib/todo-utils.js';

function getInitialData() {
  try {
    const storedTodos = Rong.storage.get("todo:todos");
    const storedFilter = Rong.storage.get("todo:filter");
    const storedLastUpdated = Rong.storage.get("todo:lastUpdated");

    if (storedTodos && Array.isArray(storedTodos) && storedTodos.length > 0) {
      console.log("[Todo] Found stored todos:", storedTodos.length);
      return {
        todos: storedTodos,
        currentFilter: storedFilter || "all",
        lastUpdated: storedLastUpdated || getCurrentTimestamp(),
      };
    } else {
      console.log("[Todo] No stored todos found, using empty state");
      return {
        todos: [],
        currentFilter: "all",
        lastUpdated: getCurrentTimestamp(),
      };
    }
  } catch (error) {
    console.error("[Todo] Error loading initial data:", error);
    return {
      todos: [],
      currentFilter: "all",
      lastUpdated: getCurrentTimestamp(),
    };
  }
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
  onLoad: function () {
    console.log("[Todo] Page loaded, initial todos:", this.data.todos.length);
  },

  onReady: function () {
    console.log("[Todo] Page ready");
  },

  onShow: function () {
    console.log("[Todo] Page shown");
    // Test bundled utilities access - use direct function calls
    if (typeof generateTodoId !== 'undefined') {
      const testId = generateTodoId();
      console.log("[Todo] Generated test UUID using bundled utilities:", testId);
    } else {
      console.log("[Todo] generateTodoId function not available");
    }
  },

  _saveToStorage: function () {
    try {
      Rong.storage.set(this.STORAGE_KEYS.TODOS, this.data.todos);
      Rong.storage.set(this.STORAGE_KEYS.FILTER, this.data.currentFilter);
      Rong.storage.set(this.STORAGE_KEYS.LAST_UPDATED, this.data.lastUpdated);
      console.log("[Todo] saveToStorage: Successfully saved to storage");
    } catch (error) {
      console.error("[Todo] saveToStorage: Error saving to storage:", error);
    }
  },

  _clearStorage: function () {
    try {
      Rong.storage.delete(this.STORAGE_KEYS.TODOS);
      Rong.storage.delete(this.STORAGE_KEYS.FILTER);
      Rong.storage.delete(this.STORAGE_KEYS.LAST_UPDATED);
      console.log("[Todo] clearStorage: Successfully cleared storage");
    } catch (error) {
      console.error("[Todo] clearStorage: Error clearing storage:", error);
    }
  },

  // Todo core functionality
  addTodo: async function (params) {
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

    console.log("[Todo] addTodo: Adding new todo:", newTodo);
    await this.setData({
      todos: [...this.data.todos, newTodo],
      lastUpdated: getCurrentTimestamp(),
    });

    // Save to storage after adding
    this._saveToStorage();

    console.log(
      "[Todo] addTodo: Todo added successfully, total todos:",
      this.data.todos.length,
    );
  },

  toggleTodo: async function (params) {
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

    await this.setData({
      todos: updatedTodos,
      lastUpdated: new Date().toISOString(),
    });

    // Save to storage after toggling
    this._saveToStorage();

    console.log("[Todo] toggleTodo: Todo toggled successfully");
  },

  deleteTodo: async function (params) {
    const { id } = params;
    if (!id) {
      console.log("[Todo] deleteTodo: No ID provided, skipping");
      return;
    }

    const targetTodo = this.data.todos.find((todo) => todo.id === id);
    console.log("[Todo] deleteTodo: Deleting todo:", targetTodo);

    const updatedTodos = this.data.todos.filter((todo) => todo.id !== id);

    await this.setData({
      todos: updatedTodos,
      lastUpdated: new Date().toISOString(),
    });

    // Save to storage after deleting
    this._saveToStorage();

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

    await this.setData({
      todos: updatedTodos,
      lastUpdated: new Date().toISOString(),
    });

    // Save to storage after clearing completed
    this._saveToStorage();

    console.log(
      "[Todo] clearCompleted: Completed todos cleared, remaining todos:",
      updatedTodos.length,
    );
  },

  setFilter: async function (params) {
    console.log("[Todo] setFilter called with params:", params);
    const { filter } = params;
    if (!filter || !["all", "active", "completed"].includes(filter)) {
      console.log("[Todo] setFilter: Invalid filter value, skipping");
      return;
    }

    console.log("[Todo] setFilter: Setting filter to:", filter);
    await this.setData({
      currentFilter: filter,
      lastUpdated: new Date().toISOString(),
    });

    // Save to storage after setting filter
    this._saveToStorage();

    console.log("[Todo] setFilter: Filter set successfully");
  },

  // Debug method (can be removed in production)
  _getStorageInfo: function () {
    try {
      const info = Rong.storage.info();
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
