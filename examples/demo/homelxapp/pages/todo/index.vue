<template>
  <section class="todoapp" v-cloak>
    <h1>todos</h1>
    <input
      class="new-todo"
      placeholder="What needs to be done?"
      v-model="newTodo"
      @keyup.enter="handleAddTodo"
      autofocus
    >
    <div class="main" v-show="data && data.todos && data.todos.length > 0">
      <input
        id="toggle-all"
        class="toggle-all"
        type="checkbox"
        :checked="todoStats.active === 0"
        @change="handleToggleAll"
      >
      <label for="toggle-all">Mark all as complete</label>
      <ul class="todo-list">
        <li
          v-for="todo in filteredTodos"
          :key="todo.id"
          :class="{ completed: todo.completed }"
        >
          <div class="view">
            <input
              class="toggle"
              type="checkbox"
              :checked="todo.completed"
              @change="toggleTodo({ id: todo.id })"
            >
            <label>{{ todo.text }}</label>
            <button
              class="destroy"
              @click="deleteTodo({ id: todo.id })"
            ></button>
          </div>
        </li>
      </ul>
    </div>
    <footer class="footer" v-show="data && data.todos && data.todos.length > 0">
      <span class="todo-count">
        <strong>{{ todoStats.active }}</strong>
        {{ todoStats.active === 1 ? 'item' : 'items' }} left
      </span>
      <ul class="filters">
        <li>
          <a
            href="#/"
            :class="{ selected: currentFilter === 'all' }"
            @click.prevent="setFilter({ filter: 'all' })"
          >All</a>
        </li>
        <li>
          <a
            href="#/active"
            :class="{ selected: currentFilter === 'active' }"
            @click.prevent="setFilter({ filter: 'active' })"
          >Active</a>
        </li>
        <li>
          <a
            href="#/completed"
            :class="{ selected: currentFilter === 'completed' }"
            @click.prevent="setFilter({ filter: 'completed' })"
          >Completed</a>
        </li>
      </ul>
      <button
        class="clear-completed"
        v-show="todoStats.completed > 0"
        @click="clearCompleted"
      >
        Clear completed
      </button>
    </footer>
  </section>

  <footer class="info">
    <p>Double-click to edit a todo</p>
  </footer>
</template>

<script>
import { ref, computed } from 'vue'

export default {
  name: 'TodoApp',
  setup() {
    const newTodo = ref('')
    const data = useLingXiaData()

    // Debug: log data changes
    console.log('[Todo] Initial data:', data.value)

    const currentFilter = computed(() => data.value?.currentFilter || 'all')

    const filteredTodos = computed(() => {
      if (!data.value?.todos) return []
      const todos = data.value.todos

      switch (currentFilter.value) {
        case 'active':
          return todos.filter(todo => !todo.completed)
        case 'completed':
          return todos.filter(todo => todo.completed)
        default:
          return todos
      }
    })

    const todoStats = computed(() => {
      const todos = data.value?.todos || []
      return {
        total: todos.length,
        completed: todos.filter(t => t.completed).length,
        active: todos.filter(t => !t.completed).length
      }
    })

    const handleAddTodo = () => {
      const text = newTodo.value.trim()
      if (!text) return

      addTodo({ text })
      newTodo.value = ''
    }

    const handleToggleAll = () => {
      const allCompleted = todoStats.value.active === 0
      data.value.todos.forEach(todo => {
        if (todo.completed !== !allCompleted) {
          toggleTodo({ id: todo.id })
        }
      })
    }

    return {
      newTodo,
      data,
      currentFilter,
      filteredTodos,
      todoStats,
      handleAddTodo,
      handleToggleAll
    }
  }
}
</script>

<style>

/* Vue.js v-cloak directive - prevents template display before mounting */
[v-cloak] {
  display: none !important;
}

/* Loading state before Vue component mounts */
.todoapp[v-cloak] {
  opacity: 0;
}

/* Vue transition animations */
.todo-item-enter-active,
.todo-item-leave-active {
  transition: all 0.3s ease;
}

.todo-item-enter-from,
.todo-item-leave-to {
  opacity: 0;
  transform: translateX(30px);
}
</style>
