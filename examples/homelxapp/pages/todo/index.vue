<template>
  <div class="todo-page">
    <section class="todoapp">
      <h1>todos</h1>
      <input
        class="new-todo"
        placeholder="What needs to be done?"
        v-model="newTodo"
        @keydown.enter="handleAddTodo"
        autofocus
      />

      <div v-if="!hasTodos" class="empty-state">
        <div class="empty-state-icon">📝</div>
        <div class="empty-state-text">No tasks yet</div>
        <div class="empty-state-hint">Add a task above to get started</div>
      </div>

      <div v-if="hasTodos" class="main">
        <input
          id="toggle-all"
          class="toggle-all"
          type="checkbox"
          :checked="todoStats.active === 0 && todos.length > 0"
          @change="handleToggleAll"
        />
        <label for="toggle-all">Mark all as complete</label>
        <ul class="todo-list">
          <li v-for="todo in filteredTodos" :key="todo.id" :class="{ completed: todo.completed }">
            <div class="view">
              <input
                class="toggle"
                type="checkbox"
                :checked="todo.completed"
                @change="toggleTodo({ id: todo.id })"
              />
              <label>{{ todo.text }}</label>
              <button
                class="destroy"
                @click="deleteTodo({ id: todo.id })"
                aria-label="Delete todo"
              />
            </div>
          </li>
        </ul>
      </div>

      <footer v-if="hasTodos" class="footer">
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
            >
              All
            </a>
          </li>
          <li>
            <a
              href="#/active"
              :class="{ selected: currentFilter === 'active' }"
              @click.prevent="setFilter({ filter: 'active' })"
            >
              Active
            </a>
          </li>
          <li>
            <a
              href="#/completed"
              :class="{ selected: currentFilter === 'completed' }"
              @click.prevent="setFilter({ filter: 'completed' })"
            >
              Completed
            </a>
          </li>
        </ul>
        <button v-if="todoStats.completed > 0" class="clear-completed" @click="clearCompleted">
          Clear completed
        </button>
      </footer>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useLingXia } from '@lingxia/core/vue';
import '../../tailwind.css';
import './index.css';

type Todo = {
  id: string;
  text: string;
  completed: boolean;
};

type TodoFilter = 'all' | 'active' | 'completed';

type PageData = {
  todos?: Todo[];
  currentFilter?: TodoFilter;
};

type PageActions = {
  data: PageData;
  addTodo(params: { text: string }): void;
  toggleTodo(params: { id: string }): void;
  deleteTodo(params: { id: string }): void;
  clearCompleted(): void;
  setFilter(params: { filter: TodoFilter }): void;
};

const {
  data,
  addTodo,
  toggleTodo,
  deleteTodo,
  clearCompleted,
  setFilter,
} = useLingXia() as PageActions;

const newTodo = ref('');

const todos = computed(() => data?.todos ?? []);
const currentFilter = computed(() => data?.currentFilter ?? 'all');

const filteredTodos = computed(() => {
  switch (currentFilter.value) {
    case 'active':
      return todos.value.filter(todo => !todo.completed);
    case 'completed':
      return todos.value.filter(todo => todo.completed);
    default:
      return todos.value;
  }
});

const todoStats = computed(() => {
  const completedCount = todos.value.filter(todo => todo.completed).length;
  return {
    total: todos.value.length,
    completed: completedCount,
    active: todos.value.length - completedCount,
  };
});

const hasTodos = computed(() => todos.value.length > 0);

function handleAddTodo() {
  const text = newTodo.value.trim();
  if (!text) return;
  addTodo({ text });
  newTodo.value = '';
}

function handleToggleAll() {
  const allCompleted = todoStats.value.active === 0;
  todos.value.forEach(todo => {
    if (todo.completed !== !allCompleted) {
      toggleTodo({ id: todo.id });
    }
  });
}
</script>
