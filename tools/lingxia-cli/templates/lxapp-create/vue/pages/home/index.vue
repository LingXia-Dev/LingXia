<script setup lang="ts">
import '../../app.css';
import { LxNavigator } from '@lingxia/components/vue';
import { useLingXia } from '@lingxia/core/vue';
import { computed, ref } from 'vue';

type PageState = { greeting?: string; greetCount?: number };
type PageActions = { greet(payload: { name: string }): void };

const { data, greet } = useLingXia<PageState, PageActions>();
const state = computed(() => data ?? { greeting: '', greetCount: 0 });
const inputName = ref('');
const canSubmit = computed(() => inputName.value.trim().length > 0);

function handleSubmit() {
  const v = inputName.value.trim();
  if (v) greet({ name: v });
}
</script>

<template>
  <div class="page">
    <div class="card">
      <img :src="'/public/AppIcon.png'" class="logo" />
      <h1 class="title">Hello, LingXia</h1>
      <p class="subtitle">Build once, run everywhere</p>

      <div class="form">
        <input v-model="inputName" placeholder="Enter your name" class="input" @keydown.enter.prevent="handleSubmit" />
        <button :disabled="!canSubmit" class="btn" @click="handleSubmit">Say Hello</button>
      </div>

      <p v-if="state.greeting" class="greeting">{{ state.greeting }}</p>

      <div class="footer">
        <LxNavigator url="https://www.lingxia.app" class="link">
          Documentation →
        </LxNavigator>
      </div>
    </div>
  </div>
</template>

<style scoped>
.page {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
}
.card {
  width: 100%;
  max-width: 360px;
  padding: 32px;
  background: #fff;
  border-radius: 20px;
  box-shadow: 0 4px 24px rgba(0,0,0,0.08);
  text-align: center;
}
.logo {
  width: 72px;
  height: 72px;
  border-radius: 16px;
  box-shadow: 0 2px 12px rgba(0,0,0,0.1);
}
.title {
  margin: 20px 0 6px;
  font-size: 26px;
  font-weight: 700;
  color: #1d1d1f;
}
.subtitle {
  margin: 0;
  font-size: 15px;
  color: #86868b;
}
.form {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-top: 28px;
}
.input {
  width: 100%;
  height: 48px;
  padding: 0 16px;
  border: 1px solid #d2d2d7;
  border-radius: 12px;
  font-size: 16px;
  outline: none;
  background: #fafafa;
  transition: border-color 0.2s;
}
.input:focus {
  border-color: #007aff;
}
.btn {
  width: 100%;
  height: 48px;
  border: none;
  border-radius: 12px;
  background: #007aff;
  color: #fff;
  font-size: 16px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.2s;
}
.btn:hover:not(:disabled) {
  background: #0066d6;
}
.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.greeting {
  margin-top: 24px;
  padding: 16px;
  background: #f0fdf4;
  border: 1px solid #bbf7d0;
  border-radius: 12px;
  color: #166534;
  font-size: 15px;
  white-space: pre-line;
  text-align: left;
}
.footer {
  margin-top: 28px;
  padding-top: 20px;
  border-top: 1px solid #f0f0f0;
}
.link {
  color: #007aff;
  font-size: 14px;
  font-weight: 500;
  text-decoration: none;
}
</style>
