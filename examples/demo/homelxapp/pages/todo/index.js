Page({
  data: {
    todos: [],
    currentFilter: "all",
    lastUpdated: new Date().toISOString(),
  },

  // Storage keys
  STORAGE_KEYS: {
    TODOS: "todo:todos",
    FILTER: "todo:filter",
    LAST_UPDATED: "todo:lastUpdated",
  },

  // Page lifecycle
  onLoad: async function () {
    console.log("[Todo] Page loaded");
    await this._loadFromStorage();
  },

  onReady: function () {
    console.log("[Todo] Page ready");
  },

  onShow: function () {
    console.log("[Todo] Page shown");
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

  _loadFromStorage: async function () {
    try {
      const storedTodos = Rong.storage.get(this.STORAGE_KEYS.TODOS);
      const storedFilter = Rong.storage.get(this.STORAGE_KEYS.FILTER);
      const storedLastUpdated = Rong.storage.get(
        this.STORAGE_KEYS.LAST_UPDATED,
      );

      if (storedTodos && Array.isArray(storedTodos) && storedTodos.length > 0) {
        console.log(
          "[Todo] loadFromStorage: Found stored todos:",
          storedTodos.length,
        );
        await this.setData({
          todos: storedTodos,
          currentFilter: storedFilter || "all",
          lastUpdated: storedLastUpdated || new Date().toISOString(),
        });
      } else {
        console.log(
          "[Todo] loadFromStorage: No stored todos found, initializing with sample data",
        );
        const defaultTodos = [
          { id: Date.now(), text: "Welcome to Todo App! 👋", completed: false },
        ];

        await this.setData({
          todos: defaultTodos,
          currentFilter: "all",
          lastUpdated: new Date().toISOString(),
        });

        this._saveToStorage();
      }
    } catch (error) {
      console.error(
        "[Todo] loadFromStorage: Error loading from storage:",
        error,
      );
      // Fallback to empty state
      await this.setData({
        todos: [],
        currentFilter: "all",
        lastUpdated: new Date().toISOString(),
      });
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
    if (!text || !text.trim()) {
      console.log("[Todo] addTodo: Empty text, skipping");
      return;
    }

    const newTodo = {
      id: Date.now(),
      text: text.trim(),
      completed: false,
    };

    console.log("[Todo] addTodo: Adding new todo:", newTodo);
    await this.setData({
      todos: [...this.data.todos, newTodo],
      lastUpdated: new Date().toISOString(),
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
