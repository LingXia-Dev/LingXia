<template>
  <div class="bg-gray-100 min-h-screen">
    <div class="px-4 py-4 space-y-3 pb-8">
      <!-- Header -->
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2">
          <div class="w-8 h-8 bg-gradient-to-br from-orange-500 to-rose-500 rounded-lg flex items-center justify-center">
            <svg viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" class="w-4 h-4">
              <rect x="3" y="5" width="18" height="14" rx="2" />
              <circle cx="9" cy="11" r="1.5" fill="white" />
              <path d="M21 15l-5-5-9 9" />
            </svg>
          </div>
          <div>
            <div class="text-base font-semibold text-gray-900">Media Swiper</div>
          </div>
        </div>
        <div class="bg-gray-900 text-emerald-400 font-mono text-xs px-3 py-1.5 rounded-lg w-[180px] truncate">
          {{ eventLog }}
        </div>
      </div>

      <!-- Stage -->
      <div class="bg-black rounded-xl overflow-hidden">
        <div
          v-if="items.length === 0"
          class="flex items-center justify-center text-white/60 text-sm"
          :style="{ aspectRatio: '16 / 10' }"
        >
          Tap "Add Image" or "Add Video" below
        </div>
        <LxMediaSwiper
          v-else
          ref="swiperRef"
          id="lx-media-swiper-demo"
          :items="items"
          :index="index"
          :autoplay="autoplay"
          :loop="loop"
          :dots="dots ? { color: '#ffffff66', activeColor: '#ffffff' } : false"
          :object-fit="objectFit"
          :animation="animation"
          :direction="direction"
          :peek="peek > 0 ? peek : undefined"
          :controls="false"
          muted
          :style="{ display: 'block', width: '100%', aspectRatio: '16 / 10', borderRadius: '12px' }"
          @change="onSwiperChange"
          @transition-end="onSwiperTransitionEnd"
          @end-reached="onSwiperEndReached"
          @tap="onSwiperTap"
          @video-ended="onSwiperVideoEnded"
          @error="onSwiperError"
        />
      </div>

      <!-- Source -->
      <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-4 space-y-3">
        <div class="text-xs text-gray-400 uppercase tracking-wider font-semibold">Add Media</div>
        <div class="grid grid-cols-2 gap-2">
          <button
            @click="pickImages?.()"
            :disabled="busy"
            class="px-3 py-2.5 rounded-lg bg-gradient-to-r from-blue-500 to-cyan-500 text-white text-sm font-medium active:scale-95 disabled:opacity-50 transition-transform"
          >
            Add Image
          </button>
          <button
            @click="pickVideos?.()"
            :disabled="busy"
            class="px-3 py-2.5 rounded-lg bg-gradient-to-r from-purple-500 to-pink-500 text-white text-sm font-medium active:scale-95 disabled:opacity-50 transition-transform"
          >
            Add Video
          </button>
        </div>
      </div>

      <!-- Controls -->
      <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-4 space-y-3">
        <div class="text-xs text-gray-400 uppercase tracking-wider font-semibold">Navigation</div>
        <div class="grid grid-cols-4 gap-2">
          <button
            @click="callPrevious"
            :disabled="items.length < 2"
            class="px-3 py-2 rounded-lg bg-gray-100 text-gray-700 text-sm font-medium active:scale-95 disabled:opacity-40"
          >
            Prev
          </button>
          <button
            @click="callNext"
            :disabled="items.length < 2"
            class="px-3 py-2 rounded-lg bg-gray-900 text-white text-sm font-medium active:scale-95 disabled:opacity-40"
          >
            Next
          </button>
          <button
            @click="removeCurrent?.()"
            :disabled="items.length === 0"
            class="px-3 py-2 rounded-lg bg-amber-50 text-amber-700 text-sm font-medium active:scale-95 disabled:opacity-40"
          >
            Remove
          </button>
          <button
            @click="clearAll?.()"
            :disabled="items.length === 0"
            class="px-3 py-2 rounded-lg bg-rose-50 text-rose-700 text-sm font-medium active:scale-95 disabled:opacity-40"
          >
            Clear
          </button>
        </div>
      </div>

      <!-- Options -->
      <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-4 space-y-3">
        <div class="text-xs text-gray-400 uppercase tracking-wider font-semibold">Options</div>

        <div class="flex items-center justify-between">
          <div>
            <div class="text-sm font-medium text-gray-900">Autoplay</div>
            <div class="text-xs text-gray-500">Cycle every 5s</div>
          </div>
          <button
            @click="toggleAutoplay?.()"
            :class="[
              'w-11 h-6 rounded-full relative transition-colors',
              autoplay ? 'bg-emerald-500' : 'bg-gray-300',
            ]"
          >
            <span
              :class="[
                'absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white shadow-sm transition-transform',
                autoplay ? 'translate-x-5' : 'translate-x-0',
              ]"
            />
          </button>
        </div>

        <div class="flex items-center justify-between">
          <div>
            <div class="text-sm font-medium text-gray-900">Loop</div>
            <div class="text-xs text-gray-500">Wrap past last item</div>
          </div>
          <button
            @click="toggleLoop?.()"
            :class="[
              'w-11 h-6 rounded-full relative transition-colors',
              loop ? 'bg-emerald-500' : 'bg-gray-300',
            ]"
          >
            <span
              :class="[
                'absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white shadow-sm transition-transform',
                loop ? 'translate-x-5' : 'translate-x-0',
              ]"
            />
          </button>
        </div>

        <div class="flex items-center justify-between">
          <div>
            <div class="text-sm font-medium text-gray-900">Dots</div>
            <div class="text-xs text-gray-500">Native page indicator</div>
          </div>
          <button
            @click="toggleDots?.()"
            :class="[
              'w-11 h-6 rounded-full relative transition-colors',
              dots ? 'bg-emerald-500' : 'bg-gray-300',
            ]"
          >
            <span
              :class="[
                'absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white shadow-sm transition-transform',
                dots ? 'translate-x-5' : 'translate-x-0',
              ]"
            />
          </button>
        </div>

        <div class="space-y-2">
          <div class="text-sm font-medium text-gray-900">Object fit</div>
          <div class="grid grid-cols-3 gap-1 bg-gray-100 rounded-lg p-1">
            <button
              v-for="fit in fits"
              :key="fit"
              @click="setObjectFit?.({ fit })"
              :class="[
                'py-1.5 px-2 rounded-md text-xs font-medium transition-colors',
                objectFit === fit ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500',
              ]"
            >
              {{ fit }}
            </button>
          </div>
        </div>

        <div class="space-y-2">
          <div class="text-sm font-medium text-gray-900">Animation</div>
          <div class="text-xs text-gray-500">v1 supports slide and none only</div>
          <div class="grid grid-cols-2 gap-1 bg-gray-100 rounded-lg p-1">
            <button
              v-for="a in animations"
              :key="a"
              @click="setAnimation?.({ animation: a })"
              :class="[
                'py-1.5 px-2 rounded-md text-xs font-medium transition-colors',
                animation === a ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500',
              ]"
            >
              {{ a }}
            </button>
          </div>
        </div>

        <div class="space-y-2">
          <div class="text-sm font-medium text-gray-900">Direction</div>
          <div class="grid grid-cols-2 gap-1 bg-gray-100 rounded-lg p-1">
            <button
              v-for="d in directions"
              :key="d"
              @click="setDirection?.({ direction: d })"
              :class="[
                'py-1.5 px-2 rounded-md text-xs font-medium transition-colors',
                direction === d ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500',
              ]"
            >
              {{ d }}
            </button>
          </div>
        </div>

        <div class="space-y-2">
          <div class="text-sm font-medium text-gray-900">Peek</div>
          <div class="text-xs text-gray-500">Adjacent page hint (px each side)</div>
          <div class="grid grid-cols-4 gap-1 bg-gray-100 rounded-lg p-1">
            <button
              v-for="value in peekPresets"
              :key="value"
              @click="setPeek?.({ value })"
              :class="[
                'py-1.5 px-2 rounded-md text-xs font-medium transition-colors',
                peek === value ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500',
              ]"
            >
              {{ value === 0 ? 'off' : `${value}px` }}
            </button>
          </div>
        </div>
      </div>

      <!-- Status -->
      <div class="bg-white rounded-xl shadow-sm border border-gray-200 px-4 py-3 flex items-center justify-between text-xs">
        <div class="text-gray-500">
          {{ items.length > 0 ? `Item ${index + 1} of ${items.length}` : 'No items' }}
        </div>
        <div class="font-mono text-gray-400">
          {{ items.length > 0 ? items[index]?.type : '—' }}
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { useLxPage } from '@lingxia/vue';
import { LxMediaSwiper } from '@lingxia/vue';
import '../../tailwind.css';

type SwiperItem = { id: string; type: 'image' | 'video'; src: string };
type ObjectFit = 'cover' | 'contain' | 'fill';
type Animation = 'slide' | 'none';
type Direction = 'horizontal' | 'vertical';

type SwiperEl = HTMLElement & {
  next(): void;
  previous(): void;
  goToIndex(index: number): void;
};

const fits: ObjectFit[] = ['cover', 'contain', 'fill'];
const animations: Animation[] = ['slide', 'none'];
const directions: Direction[] = ['horizontal', 'vertical'];
const peekPresets: number[] = [0, 16, 32, 48];

const { data, actions } = useLxPage();
const {
  pickImages,
  pickVideos,
  removeCurrent,
  clearAll,
  toggleAutoplay,
  toggleLoop,
  toggleDots,
  setObjectFit,
  setAnimation,
  setDirection,
  setPeek,
  onSwiperChange,
  onSwiperTransitionEnd,
  onSwiperEndReached,
  onSwiperTap,
  onSwiperVideoEnded,
  onSwiperError,
} = actions;

const swiperRef = ref<SwiperEl | null>(null);

const items = computed<SwiperItem[]>(() => data?.items ?? []);
const index = computed<number>(() =>
  typeof data?.index === 'number' ? data.index : 0,
);
const autoplay = computed<boolean>(() => !!data?.autoplay);
const loop = computed<boolean>(() => !!data?.loop);
const dots = computed<boolean>(() => data?.dots ?? true);
const objectFit = computed<ObjectFit>(() => (data?.objectFit as ObjectFit) ?? 'cover');
const animation = computed<Animation>(() => (data?.animation as Animation) ?? 'slide');
const direction = computed<Direction>(() => (data?.direction as Direction) ?? 'horizontal');
const peek = computed<number>(() => (typeof data?.peek === 'number' ? data.peek : 0));
const eventLog = computed<string>(() => data?.eventLog ?? 'Pick media to start');
const busy = computed<boolean>(() => !!data?.busy);

function callPrevious() {
  swiperRef.value?.previous();
}

function callNext() {
  swiperRef.value?.next();
}
</script>
