<template>
  <div class="fixed inset-0 w-screen h-screen overflow-hidden">
    <!-- Background Image - Full Screen -->
    <img
      v-if="imageUrl"
      :src="imageUrl"
      alt=""
      class="absolute inset-0 w-full h-full object-cover"
    />

    <!-- Gradient Overlay -->
    <div class="absolute inset-0 bg-gradient-to-b from-black/10 via-transparent to-black/40" />

    <!-- Content Container - Centered -->
    <div class="relative z-10 w-full h-full flex flex-col justify-center items-center px-5 py-16">
      <!-- Main Card - Apple Style Frosted Glass -->
      <div class="bg-white/80 backdrop-blur-xl rounded-2xl shadow-lg border border-white/20 p-6">
        <div class="text-center mb-6">
          <img src="/public/AppIcon.png" alt="Logo" class="w-16 h-16 mx-auto mb-3 rounded-[16px]" />
          <div class="text-[17px] font-semibold text-gray-900">LingXia</div>
          <div class="text-[13px] text-gray-500 mt-0.5">Lightweight Application Framework</div>
        </div>

        <div class="space-y-3">
          <input
            type="text"
            placeholder="Enter your name"
            v-model="name"
            @keydown.enter="handleGreet"
            class="w-full h-[44px] px-4 bg-gray-100/80 border-0 rounded-[10px] text-[17px] text-gray-900 placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500/30 transition-all"
          />

          <button
            type="button"
            @click="handleGreet"
            :disabled="!name.trim() || isSending"
            class="w-full h-[50px] bg-[#007AFF] hover:bg-[#0066CC] active:bg-[#0055B3] disabled:bg-[#007AFF]/50 disabled:cursor-not-allowed rounded-[12px] text-[17px] text-white font-semibold transition-colors"
          >
            {{ isSending ? 'Sending...' : 'Say Hello' }}
          </button>
        </div>

        <!-- Result Message -->
        <div v-if="greetingMessage" class="mt-4 p-4 bg-green-50 border border-green-200 rounded-xl">
          <div class="flex items-start gap-3">
            <div class="w-5 h-5 text-green-500 flex-shrink-0 mt-0.5">
              <svg viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z" />
              </svg>
            </div>
            <p class="text-sm text-green-700 leading-relaxed">
              {{ greetingMessage }}
            </p>
          </div>
        </div>

        <div v-if="appVersion" class="mt-1 text-left leading-none">
          <span class="text-[10px] text-gray-500 font-medium">{{ appVersion }}</span>
        </div>
      </div>

      <!-- IP Address Badge - Below Card -->
      <div v-if="ipAddress" class="mt-4 flex justify-center">
        <div class="inline-flex items-center gap-2 px-4 py-2 bg-black/20 backdrop-blur-md rounded-full text-white/90">
          <span class="w-1.5 h-1.5 bg-green-400 rounded-full animate-pulse" />
          <span class="text-xs font-medium tracking-wide">My IP </span>
          <span class="text-xs font-mono">{{ ipAddress }}</span>
        </div>
      </div>
    </div>

  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { useLxPage } from '@lingxia/vue';
import '../../tailwind.css';

type PageData = {
  greeting?: string;
  imageUrl?: string;
  ipAddr?: string;
  appVersion?: string;
};

type PageActions = {
  data: PageData;
  greet(payload: { name: string }): void;
};

const { data, actions } = useLxPage();
const { greet } = actions;
const name = ref('');
const isSending = ref(false);

const greetingMessage = computed(() => typeof data?.greeting === 'string' ? data.greeting : '');
const ipAddress = computed(() => typeof data?.ipAddr === 'string' ? data.ipAddr : '');
const imageUrl = computed(() => typeof data?.imageUrl === 'string' ? data.imageUrl : '');
const appVersion = computed(() => typeof data?.appVersion === 'string' ? data.appVersion : '');

watch(greetingMessage, (newVal) => {
  if (isSending.value && newVal) {
    isSending.value = false;
  }
});

function handleGreet() {
  const trimmed = name.value.trim();
  if (!trimmed) return;
  isSending.value = true;
  greet({ name: trimmed });
}
</script>
