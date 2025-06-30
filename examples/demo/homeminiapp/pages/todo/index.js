Page({
  data: {
    todos: [
      { id: 1, text: "Complete work report", completed: false },
      { id: 2, text: "Buy daily necessities", completed: false },
      { id: 3, text: "Exercise", completed: true }
    ],
    currentFilter: "all",
    lastUpdated: new Date().toISOString()
  },

  // Page lifecycle
  onLoad: function() {
    console.log("[Todo] Page loaded");
    this.setData({
      lastUpdated: new Date().toISOString()
    });
  },

  onReady: function() {
    console.log("[Todo] Page ready");
  },

  onShow: function() {
    console.log("[Todo] Page shown");
  },

  // Todo core functionality
  addTodo: function(params) {
    console.log("[Todo] addTodo called with params:", params);
    const { text } = params;
    if (!text || !text.trim()) {
      console.log("[Todo] addTodo: Empty text, skipping");
      return;
    }

    const newTodo = {
      id: Date.now(),
      text: text.trim(),
      completed: false
    };

    console.log("[Todo] addTodo: Adding new todo:", newTodo);
    this.setData({
      todos: [...this.data.todos, newTodo],
      lastUpdated: new Date().toISOString()
    });
    console.log("[Todo] addTodo: Todo added successfully, total todos:", this.data.todos.length + 1);
  },

  toggleTodo: function(params) {
    console.log("[Todo] toggleTodo called with params:", params);
    const { id } = params;
    if (!id) {
      console.log("[Todo] toggleTodo: No ID provided, skipping");
      return;
    }

    const targetTodo = this.data.todos.find(todo => todo.id === id);
    console.log("[Todo] toggleTodo: Target todo:", targetTodo);

    const updatedTodos = this.data.todos.map(todo =>
      todo.id === id ? { ...todo, completed: !todo.completed } : todo
    );

    this.setData({
      todos: updatedTodos,
      lastUpdated: new Date().toISOString()
    });
    console.log("[Todo] toggleTodo: Todo toggled successfully");
  },

  deleteTodo: function(params) {
    console.log("[Todo] deleteTodo called with params:", params);
    const { id } = params;
    if (!id) {
      console.log("[Todo] deleteTodo: No ID provided, skipping");
      return;
    }

    const targetTodo = this.data.todos.find(todo => todo.id === id);
    console.log("[Todo] deleteTodo: Deleting todo:", targetTodo);

    const updatedTodos = this.data.todos.filter(todo => todo.id !== id);

    this.setData({
      todos: updatedTodos,
      lastUpdated: new Date().toISOString()
    });
    console.log("[Todo] deleteTodo: Todo deleted successfully, remaining todos:", updatedTodos.length);
  },

  clearCompleted: function() {
    console.log("[Todo] clearCompleted called");
    const completedCount = this.data.todos.filter(todo => todo.completed).length;
    console.log("[Todo] clearCompleted: Clearing", completedCount, "completed todos");

    const updatedTodos = this.data.todos.filter(todo => !todo.completed);

    this.setData({
      todos: updatedTodos,
      lastUpdated: new Date().toISOString()
    });
    console.log("[Todo] clearCompleted: Completed todos cleared, remaining todos:", updatedTodos.length);
  },

  setFilter: function(params) {
    console.log("[Todo] setFilter called with params:", params);
    const { filter } = params;
    if (!filter || !['all', 'active', 'completed'].includes(filter)) {
      console.log("[Todo] setFilter: Invalid filter value, skipping");
      return;
    }

    console.log("[Todo] setFilter: Setting filter to:", filter);
    this.setData({
      currentFilter: filter,
      lastUpdated: new Date().toISOString()
    });
    console.log("[Todo] setFilter: Filter set successfully");
  }
});
