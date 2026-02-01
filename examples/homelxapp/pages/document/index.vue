<template>
  <div class="min-h-screen bg-gray-100">
    <div class="px-3 pt-6 pb-12 space-y-3">

      <!-- Options Section -->
      <div class="bg-white rounded-lg shadow-sm">
        <div class="px-4 py-3 border-b border-gray-100">
          <div class="text-base text-gray-900 font-medium">Options</div>
        </div>
        <div class="px-4 py-3">
          <label class="flex items-start cursor-pointer">
            <input
              type="checkbox"
              :checked="showMenu"
              @change="toggleShowMenu"
              class="w-5 h-5 text-blue-500 border-gray-300 rounded focus:ring-2 focus:ring-blue-500 mt-0.5"
            />
            <div class="ml-3 flex-1">
              <div class="text-sm text-gray-900 font-medium">Show Share Button</div>
              <div class="text-xs text-gray-500 mt-1">
                Only applies to PDF documents. Office documents (Word, Excel, PowerPoint) and other files always open with system default viewer.
              </div>
            </div>
          </label>
        </div>
      </div>

      <!-- PDF Section -->
      <div class="bg-white rounded-lg shadow-sm">
        <div class="px-4 py-4 border-b border-gray-100">
          <div class="text-base text-gray-900 font-medium">PDF Document</div>
        </div>

        <div class="px-4 py-4 space-y-3">
          <div>
            <div class="text-sm text-gray-600 mb-2">PDF URL:</div>
            <input
              type="text"
              :value="pdfUrl"
              @input="onPdfUrlInput({ detail: { value: ($event.target as HTMLInputElement).value } })"
              placeholder="Enter PDF URL"
              class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>

          <button
            @click="openPdf"
            :disabled="isPdfDownloading"
            :class="[
              'w-full py-3 rounded-lg text-white font-medium',
              isPdfDownloading ? 'bg-gray-400 cursor-not-allowed' : 'bg-blue-500 hover:bg-blue-600 active:bg-blue-700'
            ]"
          >
            {{ isPdfDownloading ? 'Downloading...' : 'Open PDF' }}
          </button>
        </div>
      </div>

      <!-- Office Document Section -->
      <div class="bg-white rounded-lg shadow-sm">
        <div class="px-4 py-4 border-b border-gray-100">
          <div class="text-base text-gray-900 font-medium">Office Document</div>
          <div class="text-xs text-gray-500 mt-1">Supports: doc, docx, xls, xlsx, ppt, pptx</div>
        </div>

        <div class="px-4 py-4 space-y-3">
          <div>
            <div class="text-sm text-gray-600 mb-2">Document URL:</div>
            <input
              type="text"
              :value="officeUrl"
              @input="onOfficeUrlInput({ detail: { value: ($event.target as HTMLInputElement).value } })"
              placeholder="Enter document URL"
              class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>

          <div>
            <div class="text-sm text-gray-600 mb-2">File Type:</div>
            <input
              type="text"
              :value="officeFileType"
              @input="onOfficeFileTypeInput({ detail: { value: ($event.target as HTMLInputElement).value } })"
              placeholder="e.g., docx, xlsx, pptx"
              class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
            <div class="text-xs text-gray-500 mt-1">Auto-detected from URL or enter manually</div>
          </div>

          <button
            @click="openOffice"
            :disabled="isOfficeDownloading"
            :class="[
              'w-full py-3 rounded-lg text-white font-medium',
              isOfficeDownloading ? 'bg-gray-400 cursor-not-allowed' : 'bg-blue-500 hover:bg-blue-600 active:bg-blue-700'
            ]"
          >
            {{ isOfficeDownloading ? 'Downloading...' : 'Open Document' }}
          </button>
        </div>
      </div>

    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import '../../tailwind.css';

declare function useLingXia(): any;

const {
  data,
  onPdfUrlInput,
  onOfficeUrlInput,
  onOfficeFileTypeInput,
  toggleShowMenu,
  openPdf,
  openOffice,
} = useLingXia();

const pdfUrl = computed(() => data?.pdfUrl || '');
const officeUrl = computed(() => data?.officeUrl || '');
const officeFileType = computed(() => data?.officeFileType || '');
const showMenu = computed(() => Boolean(data?.showMenu));
const isPdfDownloading = computed(() => Boolean(data?.isPdfDownloading));
const isOfficeDownloading = computed(() => Boolean(data?.isOfficeDownloading));
</script>
