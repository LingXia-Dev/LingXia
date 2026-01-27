<script setup>
import { computed, ref, watch } from 'vue';
import './styles.css';

const { data, greet } = useLingXia();
const state = computed(() => data ?? { greeting: '', greetCount: 0 });
const name = ref('');
const pending = ref(false);

watch(
  () => state.value.greetCount,
  () => {
    if (pending.value) {
      pending.value = false;
    }
  }
);

function submit() {
  const value = name.value.trim();
  if (!value) return;
  pending.value = true;
  greet({ name: value });
}
</script>

<template>
  <div class="home-screen">
    <section class="card">
      <p class="eyebrow">LingXia + Vue</p>
      <h1>Hello there 👋</h1>
      <p class="description">
        This starter wires data from the logic layer into a Vue component using the <code>useLingXia</code> API.
      </p>
      <div class="form-row">
        <input
          v-model="name"
          placeholder="Enter a name"
          @keydown.enter.prevent="submit"
        />
        <button type="button" :disabled="pending" @click="submit">
          {{ pending ? 'Sending…' : 'Greet' }}
        </button>
      </div>
      <div class="greeting">{{ state.greeting }}</div>
      <div class="meta">Invoked {{ state.greetCount }} times</div>
    </section>
  </div>
</template>
