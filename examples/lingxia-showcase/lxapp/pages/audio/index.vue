<template>
  <div v-if="data?.supported === false" class="min-h-screen bg-surface-100 px-3 py-3">
    <div class="bg-surface rounded-lg shadow-sm px-4 py-4 text-sm text-gray-700" data-testid="audio-unsupported">
      lx.audio is not available on this platform.
    </div>
  </div>
  <div v-else class="min-h-screen bg-surface-100 overflow-y-auto">
    <div class="px-3 py-3 pb-12 space-y-3">
      <div class="bg-surface rounded-lg shadow-sm">
        <div class="px-4 py-4 border-b border-line-100">
          <div class="text-base text-gray-900 font-medium">Player</div>
          <div class="text-xs text-gray-500 mt-1">
            lx.audio.createPlayer plays a native queue: two generated tones, then a remote MP3.
          </div>
        </div>
        <div class="px-4 py-4 space-y-3">
          <div class="text-sm text-gray-900 font-medium" data-testid="audio-track">{{ data?.trackTitle || 'Nothing loaded' }}</div>
          <div class="h-1.5 rounded bg-surface-200 overflow-hidden">
            <div class="h-full bg-blue-500" :style="{ width: `${Math.round((data?.progress ?? 0) * 100)}%` }" />
          </div>
          <div class="flex justify-between text-xs text-gray-500">
            <span data-testid="audio-position">{{ data?.positionText || '--:--' }}</span>
            <span data-testid="audio-status">{{ data?.status || 'idle' }}</span>
            <span>{{ data?.durationText || '--:--' }}</span>
          </div>
          <div class="grid grid-cols-4 gap-2">
            <button :class="secondary" @click="previous">Prev</button>
            <button
              data-testid="audio-toggle"
              class="py-2.5 rounded-lg bg-blue-500 text-white text-sm font-medium"
              @click="togglePlay"
            >
              {{ data?.status === 'playing' ? 'Pause' : 'Play' }}
            </button>
            <button :class="secondary" @click="next">Next</button>
            <button :class="secondary" @click="stop">Stop</button>
          </div>
          <div class="grid grid-cols-2 gap-2">
            <button :class="secondary" @click="seekBy({ deltaMs: -1000 })">−1 s</button>
            <button :class="secondary" @click="seekBy({ deltaMs: 1000 })">+1 s</button>
          </div>
        </div>
      </div>

      <div class="bg-surface rounded-lg shadow-sm">
        <div class="px-4 py-4 border-b border-line-100">
          <div class="text-sm text-gray-900 font-medium">Queue</div>
        </div>
        <div>
          <div
            v-for="(track, index) in data?.tracks ?? []"
            :key="track.id"
            :class="['px-4 py-3 text-sm cursor-pointer border-t border-line-100 first:border-t-0', index === data?.index ? 'text-blue-600 font-medium' : 'text-gray-700']"
            @click="skipTo({ index })"
          >
            {{ track.title }}
          </div>
        </div>
      </div>

      <div class="bg-surface rounded-lg shadow-sm">
        <div class="px-4 py-4 border-b border-line-100">
          <div class="text-sm text-gray-900 font-medium">Settings</div>
          <div class="text-xs text-gray-500 mt-1" data-testid="audio-route">
            Route: {{ data?.routeText || 'unknown' }}
          </div>
        </div>
        <div class="px-4 py-4 space-y-3">
          <div class="grid grid-cols-3 gap-2">
            <button :class="secondary" @click="cycleRate">Rate {{ data?.rate ?? 1 }}×</button>
            <button :class="secondary" @click="cycleRepeat">Repeat {{ data?.repeat || 'off' }}</button>
            <button :class="secondary" @click="toggleMute">{{ data?.muted ? 'Unmute' : 'Mute' }}</button>
          </div>
          <div class="grid grid-cols-3 gap-2">
            <button
              v-for="value in [0.25, 0.5, 1]"
              :key="value"
              :class="['py-2.5 rounded-lg text-sm font-medium', (data?.volume ?? 1) === value ? 'bg-blue-500 text-white' : 'bg-surface-200 text-gray-900']"
              @click="setVolume({ volume: value })"
            >
              Volume {{ Math.round(value * 100) }}%
            </button>
          </div>
          <button v-if="data?.earpiece" :class="['w-full', secondary]" @click="toggleOutput">
            {{ data?.output === 'earpiece' ? 'Use speaker' : 'Use earpiece' }}
          </button>
        </div>
      </div>

      <div class="bg-surface rounded-lg shadow-sm">
        <div class="px-4 py-4 border-b border-line-100">
          <div class="text-sm text-gray-900 font-medium">Sound effects</div>
          <div class="text-xs text-gray-500 mt-1">
            lx.audio.loadSound preloads a short beep; each tap is an independent voice that mixes with the player.
          </div>
        </div>
        <div class="px-4 py-4 grid grid-cols-4 gap-2">
          <button :disabled="!data?.soundReady" :class="secondary" @click="playSound({ pan: -1 })">Left</button>
          <button
            data-testid="audio-sound"
            :disabled="!data?.soundReady"
            class="py-2.5 rounded-lg bg-blue-500 text-white text-sm font-medium disabled:opacity-50"
            @click="playSound({ pan: 0 })"
          >
            Beep
          </button>
          <button :disabled="!data?.soundReady" :class="secondary" @click="playSound({ pan: 1 })">Right</button>
          <button :class="secondary" @click="stopSounds">Stop all</button>
        </div>
      </div>

      <div class="bg-surface rounded-lg shadow-sm">
        <div class="px-4 py-4 border-b border-line-100">
          <div class="text-sm text-gray-900 font-medium">Events</div>
        </div>
        <div class="px-4 py-3 space-y-1 text-xs text-gray-500 font-mono" data-testid="audio-events">
          <div v-if="!data?.eventLog?.length">No events yet</div>
          <div v-for="(line, i) in data?.eventLog ?? []" :key="i">{{ line }}</div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useLxPage } from '@lingxia/vue';
import '../../tailwind.css';

type PageData = {
  supported?: boolean;
  earpiece?: boolean;
  status?: string;
  trackTitle?: string;
  index?: number;
  tracks?: Array<{ id: string; title: string }>;
  positionText?: string;
  durationText?: string;
  progress?: number;
  rate?: number;
  repeat?: string;
  muted?: boolean;
  volume?: number;
  output?: string;
  routeText?: string;
  soundReady?: boolean;
  eventLog?: string[];
};

type PageActions = {
  togglePlay(): void;
  stop(): void;
  previous(): void;
  next(): void;
  skipTo(params: { index: number }): void;
  seekBy(params: { deltaMs: number }): void;
  cycleRate(): void;
  cycleRepeat(): void;
  toggleMute(): void;
  setVolume(params: { volume: number }): void;
  toggleOutput(): void;
  playSound(params: { pan: number }): void;
  stopSounds(): void;
};

const secondary = 'py-2.5 rounded-lg bg-surface-200 text-gray-900 text-sm font-medium';

const { data, actions } = useLxPage<PageData, PageActions>();
const {
  togglePlay, stop, previous, next, skipTo, seekBy, cycleRate, cycleRepeat,
  toggleMute, setVolume, toggleOutput, playSound, stopSounds,
} = actions;
</script>
