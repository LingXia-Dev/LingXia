<template>
  <div class="bg-surface-100 min-h-screen">
    <div v-if="!video" class="flex items-center justify-center min-h-screen">
      <div class="text-gray-500">Loading video...</div>
    </div>

    <div v-else class="px-4 py-4 space-y-3 pb-6" data-testid="video-page">
      <!-- Header -->
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2">
          <div class="w-8 h-8 bg-linear-to-br from-blue-500 to-purple-600 rounded-lg flex items-center justify-center">
            <svg viewBox="0 0 24 24" fill="white" class="w-4 h-4">
              <polygon points="5 3 19 12 5 21 5 3" />
            </svg>
          </div>
          <div>
            <div class="text-base font-semibold text-gray-900">Native Video</div>
          </div>
        </div>
        <div data-testid="video-event" class="bg-surface-900 text-green-400 font-mono text-xs px-3 py-1.5 rounded-lg w-[180px] truncate">
          {{ eventLog }}
        </div>
        <div data-testid="island-playing" class="sr-only">{{ islandPlaying ? 'yes' : 'no' }}</div>
        <div data-testid="native-press-source" class="sr-only">{{ nativePressSource }}</div>
      </div>

      <div class="flex items-center justify-between gap-3 rounded-xl border border-blue-200 bg-blue-50 px-3 py-2.5">
        <div class="min-w-0">
          <div class="text-xs font-semibold text-blue-900">H5 → native video menu</div>
          <div data-testid="native-menu-js-result" class="text-[11px] leading-4 text-blue-700">{{ nativeMenuResult }}</div>
        </div>
        <button
          type="button"
          data-testid="native-menu-toggle"
          :aria-label="nativeMenuOpen ? 'Close native menu' : 'Open native menu'"
          :aria-expanded="nativeMenuOpen"
          class="flex shrink-0 items-center gap-2 rounded-lg bg-blue-600 px-3 py-2 text-xs font-semibold text-white active:scale-95"
          @click="toggleNativeMenu"
        >
          <svg viewBox="0 0 20 20" fill="none" class="h-4 w-4" aria-hidden="true">
            <path d="M3 5h14M3 10h14M3 15h14" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
          </svg>
          {{ nativeMenuOpen ? 'Close' : 'Menu' }}
        </button>
        <span data-testid="native-menu-state" class="sr-only">{{ nativeMenuOpen ? 'open' : 'closed' }}</span>
      </div>

      <div class="bg-black rounded-xl overflow-hidden">
        <LxNativeRoot id="video-native-root" class="block w-full" :style="{ aspectRatio: '16 / 9' }">
          <LxVideo
            :id="video.id"
            :src="video.src"
            :poster="video.poster"
            :qualities="video.qualities"
            :playback-rates="video.playbackRates"
            :autoplay="video.autoplay ?? Boolean(video.src)"
            controls
            volume="0.8"
            class="block w-full rounded-lg bg-black"
            :style="{ aspectRatio: '16 / 9', borderRadius: '12px' }"
            @playing="onNativePlaying"
            @error="onError"
            @pause="onNativePause"
            @stop="onStop"
            @ended="onEnded"
            @waiting="onWaiting"
            @time-update="onTimeUpdate"
            @fullscreen-change="onFullscreenChange"
            @quality-change="onQualityChange"
            @rate-change="onRateChange"
          />
          <LxNativeCover
            v-if="nativeMenuOpen"
            id="video-native-cover"
            automation-id="video-native-cover"
            scrim="none"
            role="presentation"
          >
            <LxNativeView
              id="video-native-menu"
              automation-id="video-native-menu"
              role="menu"
              aria-label="Native video menu"
              class="absolute right-3 top-3 border border-slate-500 bg-slate-900"
              :style="{ width: '240px', height: '144px', borderRadius: '14px', backgroundColor: '#0f172a', borderColor: '#64748b', borderWidth: '1px' }"
            >
              <LxNativeText
                id="video-native-menu-title"
                class="absolute left-4 right-4 top-4 text-sm font-semibold text-white"
                :font-size="14"
                :font-weight="600"
                color="#ffffff"
                :max-lines="1"
              >Native menu</LxNativeText>
              <LxNativeText
                id="video-native-menu-detail"
                class="absolute left-4 right-4 top-11 text-xs text-slate-300"
                :font-size="11"
                color="#cbd5e1"
                :max-lines="1"
              >NativeView above native video</LxNativeText>
              <LxNativeButton
                id="video-native-menu-more"
                automation-id="video-native-menu-more"
                label="More"
                icon="more"
                intent="accent"
                emphasis="primary"
                size="compact"
                aria-label="More native menu actions"
                class="absolute bottom-4 left-4"
                :style="{ width: '96px', height: '40px', borderRadius: '10px' }"
                @press="onNativeMenuMore"
              />
              <LxNativeButton
                id="video-native-menu-close"
                automation-id="video-native-menu-close"
                label="Close"
                icon="close"
                emphasis="secondary"
                size="compact"
                aria-label="Close native menu"
                class="absolute bottom-4 right-4"
                :style="{ width: '96px', height: '40px', borderRadius: '10px' }"
                @press="onNativeMenuClose"
              />
            </LxNativeView>
          </LxNativeCover>
        </LxNativeRoot>
      </div>

      <!-- Controls -->
      <div class="bg-surface/80 backdrop-blur-xl rounded-2xl shadow-lg border border-white/20 p-5">
        <div class="text-xs text-gray-400 uppercase tracking-wider mb-4 font-semibold">Playback Controls</div>

        <div class="flex items-center justify-center gap-4 mb-5">
          <button
            @click="seekBackward(SEEK_STEP_SECONDS)"
            class="w-12 h-12 rounded-full bg-surface-100 hover:bg-surface-200 active:scale-95 transition-all flex items-center justify-center"
          >
            <svg viewBox="0 0 24 24" fill="none" class="w-5 h-5 text-gray-600">
              <path d="M12 5V1L7 6l5 5V7c3.31 0 6 2.69 6 6s-2.69 6-6 6-6-2.69-6-6H4c0 4.42 3.58 8 8 8s8-3.58 8-8-3.58-8-8-8z" fill="currentColor" />
              <text x="12" y="14" text-anchor="middle" font-size="5" fill="currentColor" font-weight="bold">{{ SEEK_STEP_SECONDS }}</text>
            </svg>
          </button>

          <button
            data-testid="video-play"
            @click="play()"
            class="w-16 h-16 rounded-full bg-linear-to-b from-green-400 to-green-600 hover:from-green-500 hover:to-green-700 active:scale-95 transition-all flex items-center justify-center shadow-lg shadow-green-500/30"
          >
            <svg viewBox="0 0 24 24" fill="white" class="w-7 h-7 ml-1">
              <polygon points="5 3 19 12 5 21 5 3" />
            </svg>
          </button>

          <button
            data-testid="video-pause"
            @click="pause()"
            class="w-14 h-14 rounded-full bg-linear-to-b from-surface-700 to-surface-900 hover:from-surface-600 hover:to-surface-800 active:scale-95 transition-all flex items-center justify-center shadow-lg shadow-gray-900/30"
          >
            <svg viewBox="0 0 24 24" fill="white" class="w-6 h-6">
              <rect x="6" y="4" width="4" height="16" rx="1" />
              <rect x="14" y="4" width="4" height="16" rx="1" />
            </svg>
          </button>

          <button
            @click="seekForward(SEEK_STEP_SECONDS)"
            class="w-12 h-12 rounded-full bg-surface-100 hover:bg-surface-200 active:scale-95 transition-all flex items-center justify-center"
          >
            <svg viewBox="0 0 24 24" fill="none" class="w-5 h-5 text-gray-600">
              <path d="M12 5V1l5 5-5 5V7c-3.31 0-6 2.69-6 6s2.69 6 6 6 6-2.69 6-6h2c0 4.42-3.58 8-8 8s-8-3.58-8-8 3.58-8 8-8z" fill="currentColor" />
              <text x="12" y="14" text-anchor="middle" font-size="5" fill="currentColor" font-weight="bold">{{ SEEK_STEP_SECONDS }}</text>
            </svg>
          </button>
        </div>

        <div class="flex items-center justify-center gap-3">
          <button
            data-testid="video-stop"
            @click="stop()"
            class="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-red-50 hover:bg-red-100 active:scale-98 transition-all"
          >
            <svg viewBox="0 0 24 24" fill="currentColor" class="w-4 h-4 text-red-500 dark:text-red-400">
              <rect x="6" y="6" width="12" height="12" rx="2" />
            </svg>
            <span class="text-sm font-medium text-red-600 dark:text-red-400">Stop</span>
          </button>

          <button
            @click="requestFullScreen()"
            class="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-indigo-50 hover:bg-indigo-100 active:scale-98 transition-all"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="w-4 h-4 text-indigo-500 dark:text-indigo-400">
              <path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3" />
            </svg>
            <span class="text-sm font-medium text-indigo-600 dark:text-indigo-400">Fullscreen</span>
          </button>
        </div>
      </div>

      <!-- Info Card -->
      <div class="bg-blue-50 border border-blue-200 rounded-xl p-3">
        <div class="flex gap-2">
          <div class="text-blue-500 dark:text-blue-400 mt-0.5 shrink-0">
            <svg viewBox="0 0 24 24" fill="currentColor" class="w-4 h-4">
              <circle cx="12" cy="12" r="10" opacity="0.2" />
              <path d="M12 16v-4m0-4h.01M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z" fill="none" stroke="currentColor" stroke-width="2" />
            </svg>
          </div>
          <div class="text-xs text-blue-700 dark:text-blue-400 leading-relaxed">
            Video config comes from <code class="bg-blue-100 px-1 py-0.5 rounded text-blue-800 dark:text-blue-400">data.videos</code> in <code class="bg-blue-100 px-1 py-0.5 rounded text-blue-800 dark:text-blue-400">pages/video/index.js</code>.
            Quality and playbackRate are passed to the native player.
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { useLxPage } from '@lingxia/vue';
import {
  LxNativeButton,
  LxNativeCover,
  LxNativeRoot,
  LxNativeText,
  LxNativeView,
  LxVideo,
} from '@lingxia/vue';
import '../../tailwind.css';

type VideoConfig = {
  id: string;
  src: string;
  poster?: string;
  autoplay?: boolean;
  qualities?: Array<{ label: string; url?: string }>;
  playbackRates?: number[];
};

type PageData = {
  videos?: VideoConfig[];
};

const {
  data, actions,
} = useLxPage();
const {
  play,
  pause,
  onError,
  stop,
  seek,
  requestFullScreen,
  onPlaying,
  onPause,
  onStop,
  onEnded,
  onWaiting,
  onTimeUpdate,
  onFullscreenChange,
  onQualityChange,
  onRateChange,
} = actions;

const SEEK_STEP_SECONDS = 10;
const eventLog = computed(() => data?.eventLog || 'Ready');
const currentTime = computed(() => (typeof data?.currentTime === 'number' ? data.currentTime : 0));
const duration = computed(() => (typeof data?.duration === 'number' ? data.duration : 0));
const islandPlaying = ref(false);
const nativePressSource = ref('none');
const nativeMenuOpen = ref(false);
const nativeMenuResult = ref('Tap Menu to mount native actions above the video.');

const video = computed(() => data?.videos?.[0]);

function onNativePlaying(payload: unknown) {
  islandPlaying.value = true;
  onPlaying(payload);
}

function onNativePause(payload: unknown) {
  islandPlaying.value = false;
  onPause(payload);
}

function toggleNativeMenu() {
  nativeMenuOpen.value = !nativeMenuOpen.value;
  nativeMenuResult.value = nativeMenuOpen.value
    ? 'H5 mounted the native menu.'
    : 'H5 removed the native menu.';
}

function onNativeMenuMore(payload: { source?: string }) {
  nativePressSource.value = payload?.source || 'unknown';
  nativeMenuOpen.value = false;
  nativeMenuResult.value = 'More handled by View JS.';
}

function onNativeMenuClose(payload: { source?: string }) {
  nativePressSource.value = payload?.source || 'unknown';
  nativeMenuOpen.value = false;
  nativeMenuResult.value = 'Close handled by View JS.';
}

function seekBackward(seconds: number) {
  const newTime = Math.max(0, currentTime.value - seconds);
  seek(newTime);
}

function seekForward(seconds: number) {
  const maxTime = duration.value > 0 ? duration.value : Number.POSITIVE_INFINITY;
  const newTime = Math.min(maxTime, currentTime.value + seconds);
  seek(newTime);
}
</script>
