<template>
  <div class="min-h-screen bg-surface-100 overflow-y-auto">
    <div class="px-3 py-3 pb-12 space-y-3">
      <div class="bg-surface rounded-lg shadow-sm">
        <div class="px-4 py-4 border-b border-line-100">
          <div class="text-base text-gray-900 font-medium">Clipboard</div>
          <div class="text-xs text-gray-500 mt-1">
            lx.clipboard reads and writes the system clipboard. Write never toasts.
          </div>
        </div>
        <div class="px-4 py-3 text-sm text-gray-700" data-testid="clipboard-status">
          {{ data?.statusText || 'Ready' }}
        </div>
        <div class="px-4 pb-3 text-xs text-gray-500" data-testid="clipboard-types">
          types: {{ data?.typesText || 'Not peeked' }}
        </div>
      </div>

      <div class="bg-surface rounded-lg shadow-sm">
        <div class="px-4 py-4 border-b border-line-100">
          <div class="text-sm text-gray-900 font-medium">Text</div>
          <div class="text-xs text-gray-500 mt-1">
            writeText / readText. An empty string is a valid write, not clear().
          </div>
        </div>
        <div class="px-4 py-4 space-y-3">
          <textarea
            data-testid="clipboard-draft"
            v-model="draft"
            rows="3"
            class="w-full px-3 py-2 border border-line-300 rounded-md text-sm"
          />
          <div class="grid grid-cols-2 gap-3">
            <button
              data-testid="clipboard-write-text"
              @click="writeText({ text: draft })"
              class="py-3 rounded-lg bg-blue-500 text-white font-medium"
            >
              Write text
            </button>
            <button
              data-testid="clipboard-read-text"
              @click="readText"
              class="py-3 rounded-lg bg-surface-900 text-white font-medium"
            >
              Read text
            </button>
          </div>
          <button
            data-testid="clipboard-write-typed"
            @click="writeTypedText({ text: draft })"
            class="w-full py-3 rounded-lg bg-surface-200 text-gray-900 font-medium"
          >
            Write typed item
          </button>
          <div
            v-if="data?.readTextValue"
            class="rounded bg-surface-50 px-3 py-2 text-xs text-gray-500 break-all"
          >
            {{ data.readTextValue }}
          </div>
        </div>
      </div>

      <div class="bg-surface rounded-lg shadow-sm">
        <div class="px-4 py-4 border-b border-line-100">
          <div class="text-sm text-gray-900 font-medium">Image</div>
          <div class="text-xs text-gray-500 mt-1">
            Write a 1×1 sample PNG, or pick one with lx.chooseMedia.
          </div>
        </div>
        <div class="px-4 py-4 space-y-3">
          <div class="grid grid-cols-2 gap-3">
            <button
              data-testid="clipboard-write-sample-image"
              @click="writeSampleImage"
              class="py-3 rounded-lg bg-blue-500 text-white font-medium"
            >
              Write sample PNG
            </button>
            <button
              @click="chooseAndWriteImage"
              class="py-3 rounded-lg bg-surface-900 text-white font-medium"
            >
              Choose image
            </button>
          </div>
          <div v-if="data?.imagePath" class="space-y-2">
            <img
              :src="data.imagePath"
              alt="Clipboard image"
              class="max-h-40 rounded border border-line-200"
            />
            <div class="text-xs text-gray-500 break-all">{{ data.imagePath }}</div>
          </div>
        </div>
      </div>

      <div class="bg-surface rounded-lg shadow-sm">
        <div class="px-4 py-4 border-b border-line-100">
          <div class="text-sm text-gray-900 font-medium">Inspect</div>
          <div class="text-xs text-gray-500 mt-1">
            Peek types without the payload, read every representation, or clear.
          </div>
        </div>
        <div class="px-4 py-4 grid grid-cols-3 gap-3">
          <button
            data-testid="clipboard-types"
            @click="peekTypes"
            class="py-3 rounded-lg bg-surface-200 text-gray-900 font-medium"
          >
            Types
          </button>
          <button
            data-testid="clipboard-read"
            @click="readAll"
            class="py-3 rounded-lg bg-blue-500 text-white font-medium"
          >
            Read
          </button>
          <button
            data-testid="clipboard-clear"
            @click="clearClipboard"
            class="py-3 rounded-lg bg-surface-900 text-white font-medium"
          >
            Clear
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useLxPage } from '@lingxia/vue';
import '../../tailwind.css';

type PageData = {
  statusText?: string;
  typesText?: string;
  readTextValue?: string;
  imagePath?: string;
};

type PageActions = {
  writeText(params: { text: string }): void;
  readText(): void;
  writeTypedText(params: { text: string }): void;
  writeSampleImage(): void;
  chooseAndWriteImage(): void;
  readAll(): void;
  peekTypes(): void;
  clearClipboard(): void;
};

const { data, actions } = useLxPage<PageData, PageActions>();
const {
  writeText,
  readText,
  writeTypedText,
  writeSampleImage,
  chooseAndWriteImage,
  readAll,
  peekTypes,
  clearClipboard,
} = actions;
const draft = ref('Hello from lx.clipboard');
</script>
