<script setup lang="ts">
import '../../app.css';
import { LxNavigator, useLxPage } from '@lingxia/vue';
import { computed, ref } from 'vue';

import type { HomePage } from './contract';

const { data, actions } = useLxPage<HomePage['data'], HomePage['actions']>();
const greeting = computed(() => data.greeting);
const inputName = ref('');

function handleSubmit() {
  const name = inputName.value.trim();
  if (name) actions.greet({ name });
}
</script>

<template>
  <main class="page">
    <section class="card">
      <h1>Hello, LingXia</h1>
      <div class="form">
        <input
          v-model="inputName"
          class="input"
          data-testid="home-name"
          placeholder="Enter your name"
          @keydown.enter.prevent="handleSubmit"
        />
        <button class="btn" data-testid="home-greet" :disabled="!inputName.trim()" @click="handleSubmit">Say Hello</button>
      </div>
      <p v-if="greeting" class="greeting" data-testid="home-greeting">{{ greeting }}</p>
      <LxNavigator url="https://www.lingxia.app" class="link">lingxia.app →</LxNavigator>
    </section>
  </main>
</template>
