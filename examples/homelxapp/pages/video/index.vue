<template>
  <div class="bg-gray-100 min-h-screen">
    <div v-if="!video" class="flex items-center justify-center min-h-screen">
      <div class="text-gray-500">Loading video...</div>
    </div>

    <div v-else class="px-4 py-4 space-y-3 pb-6">
      <!-- Header -->
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2">
          <div class="w-8 h-8 bg-gradient-to-br from-blue-500 to-purple-600 rounded-lg flex items-center justify-center">
            <svg viewBox="0 0 24 24" fill="white" class="w-4 h-4">
              <polygon points="5 3 19 12 5 21 5 3" />
            </svg>
          </div>
          <div>
            <div class="text-base font-semibold text-gray-900">Native Video</div>
          </div>
        </div>
        <div class="bg-gray-900 text-green-400 font-mono text-xs px-3 py-1.5 rounded-lg w-[180px] truncate">
          {{ eventLog }}
        </div>
      </div>

      <div class="bg-black rounded-xl overflow-hidden">
        <LxVideo
          :id="video.id"
          :src="video.src"
          :poster="video.poster"
          :qualities="video.qualities"
          :playback-rates="video.playbackRates"
          autoplay
          controls
          volume="0.8"
          class="block w-full rounded-lg bg-black"
          :style="{ aspectRatio: '16 / 9', borderRadius: '12px' }"
          @playing="onPlaying"
          @pause="onPause"
          @stop="onStop"
          @ended="onEnded"
          @waiting="onWaiting"
          @time-update="onTimeUpdate"
          @fullscreen-change="onFullscreenChange"
          @quality-change="onQualityChange"
          @rate-change="onRateChange"
        />
      </div>

      <!-- Controls -->
      <div class="bg-white/80 backdrop-blur-xl rounded-2xl shadow-lg border border-white/20 p-5">
        <div class="text-xs text-gray-400 uppercase tracking-wider mb-4 font-semibold">Playback Controls</div>

        <div class="flex items-center justify-center gap-4 mb-5">
          <button
            @click="seekBackward(SEEK_STEP_SECONDS)"
            class="w-12 h-12 rounded-full bg-gray-100 hover:bg-gray-200 active:scale-95 transition-all flex items-center justify-center"
          >
            <svg viewBox="0 0 24 24" fill="none" class="w-5 h-5 text-gray-600">
              <path d="M12 5V1L7 6l5 5V7c3.31 0 6 2.69 6 6s-2.69 6-6 6-6-2.69-6-6H4c0 4.42 3.58 8 8 8s8-3.58 8-8-3.58-8-8-8z" fill="currentColor" />
              <text x="12" y="14" text-anchor="middle" font-size="5" fill="currentColor" font-weight="bold">{{ SEEK_STEP_SECONDS }}</text>
            </svg>
          </button>

          <button
            @click="play()"
            class="w-16 h-16 rounded-full bg-gradient-to-b from-green-400 to-green-600 hover:from-green-500 hover:to-green-700 active:scale-95 transition-all flex items-center justify-center shadow-lg shadow-green-500/30"
          >
            <svg viewBox="0 0 24 24" fill="white" class="w-7 h-7 ml-1">
              <polygon points="5 3 19 12 5 21 5 3" />
            </svg>
          </button>

          <button
            @click="pause()"
            class="w-14 h-14 rounded-full bg-gradient-to-b from-gray-700 to-gray-900 hover:from-gray-600 hover:to-gray-800 active:scale-95 transition-all flex items-center justify-center shadow-lg shadow-gray-900/30"
          >
            <svg viewBox="0 0 24 24" fill="white" class="w-6 h-6">
              <rect x="6" y="4" width="4" height="16" rx="1" />
              <rect x="14" y="4" width="4" height="16" rx="1" />
            </svg>
          </button>

          <button
            @click="seekForward(SEEK_STEP_SECONDS)"
            class="w-12 h-12 rounded-full bg-gray-100 hover:bg-gray-200 active:scale-95 transition-all flex items-center justify-center"
          >
            <svg viewBox="0 0 24 24" fill="none" class="w-5 h-5 text-gray-600">
              <path d="M12 5V1l5 5-5 5V7c-3.31 0-6 2.69-6 6s2.69 6 6 6 6-2.69 6-6h2c0 4.42-3.58 8-8 8s-8-3.58-8-8 3.58-8 8-8z" fill="currentColor" />
              <text x="12" y="14" text-anchor="middle" font-size="5" fill="currentColor" font-weight="bold">{{ SEEK_STEP_SECONDS }}</text>
            </svg>
          </button>
        </div>

        <div class="flex items-center justify-center gap-3">
          <button
            @click="stop()"
            class="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-red-50 hover:bg-red-100 active:scale-98 transition-all"
          >
            <svg viewBox="0 0 24 24" fill="currentColor" class="w-4 h-4 text-red-500">
              <rect x="6" y="6" width="12" height="12" rx="2" />
            </svg>
            <span class="text-sm font-medium text-red-600">Stop</span>
          </button>

          <button
            @click="requestFullScreen()"
            class="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-indigo-50 hover:bg-indigo-100 active:scale-98 transition-all"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="w-4 h-4 text-indigo-500">
              <path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3" />
            </svg>
            <span class="text-sm font-medium text-indigo-600">Fullscreen</span>
          </button>
        </div>
      </div>

      <!-- Info Card -->
      <div class="bg-blue-50 border border-blue-200 rounded-xl p-3">
        <div class="flex gap-2">
          <div class="text-blue-500 mt-0.5 flex-shrink-0">
            <svg viewBox="0 0 24 24" fill="currentColor" class="w-4 h-4">
              <circle cx="12" cy="12" r="10" opacity="0.2" />
              <path d="M12 16v-4m0-4h.01M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z" fill="none" stroke="currentColor" stroke-width="2" />
            </svg>
          </div>
          <div class="text-xs text-blue-700 leading-relaxed">
            Video config comes from <code class="bg-blue-100 px-1 py-0.5 rounded text-blue-800">data.videos</code> in <code class="bg-blue-100 px-1 py-0.5 rounded text-blue-800">pages/video/index.js</code>.
            Quality and playbackRate are passed to the native player.
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useLingXia } from '@lingxia/vue';
import { LxVideo } from '@lingxia/vue';
import '../../tailwind.css';

type VideoConfig = {
  id: string;
  src: string;
  poster?: string;
  qualities?: Array<{ label: string; url?: string }>;
  playbackRates?: number[];
};

type PageData = {
  videos?: VideoConfig[];
};

const {
  data,
  play,
  pause,
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
} = useLingXia() as {
  data?: Record<string, unknown>;
  play: () => void;
  pause: () => void;
  stop: () => void;
  seek: (position: number) => void;
  requestFullScreen: () => void;
  onPlaying: (e: Event) => void;
  onPause: (e: Event) => void;
  onStop: (e: Event) => void;
  onEnded: (e: Event) => void;
  onWaiting: (e: Event) => void;
  onTimeUpdate: (e: Event) => void;
  onFullscreenChange: (e: Event) => void;
  onQualityChange: (e: Event) => void;
  onRateChange: (e: Event) => void;
};

const SEEK_STEP_SECONDS = 10;
const eventLog = computed(() => data?.eventLog || 'Ready');
const currentTime = computed(() => (typeof data?.currentTime === 'number' ? data.currentTime : 0));
const duration = computed(() => (typeof data?.duration === 'number' ? data.duration : 0));

const video = computed(() => data?.videos?.[0]);

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
