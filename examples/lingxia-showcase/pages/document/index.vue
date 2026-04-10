<template>
  <div class="min-h-screen bg-gray-100">
    <div class="px-3 pt-6 pb-12 space-y-3">

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
                Only applies to PDF documents. Office documents always open with system default viewer.
              </div>
            </div>
          </label>
        </div>
      </div>

      <div class="bg-white rounded-lg shadow-sm">
        <div class="px-4 py-4 border-b border-gray-100">
          <div class="text-base text-gray-900 font-medium">PDF via fetch()</div>
          <div class="text-xs text-gray-500 mt-1">Standard fetch validation, then open the resolved PDF URL in-app.</div>
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
            {{ isPdfDownloading ? 'Fetching PDF...' : 'Fetch and Preview PDF' }}
          </button>
        </div>
      </div>

      <div class="bg-white rounded-lg shadow-sm">
        <div class="px-4 py-4 border-b border-gray-100">
          <div class="text-base text-gray-900 font-medium">Office via lx.downloadFile()</div>
          <div class="text-xs text-gray-500 mt-1">Supports: doc, docx, xls, xlsx, ppt, pptx. Promise-like task with progress and pause/continue.</div>
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
            {{ officePrimaryButtonText }}
          </button>
          <div class="rounded-xl border border-blue-100 bg-blue-50/70 p-3">
            <div class="flex items-center justify-between text-xs text-blue-700">
              <span>Transfer Progress</span>
              <span>{{ officeProgressKnown ? `${Math.round(officeDownloadProgress)}%` : 'Streaming' }}</span>
            </div>
            <div class="mt-2 h-2 overflow-hidden rounded-full bg-blue-100">
              <div
                :class="[
                  'h-full rounded-full bg-blue-500 transition-all duration-300',
                  officeProgressKnown ? '' : 'animate-pulse'
                ]"
                :style="{ width: officeProgressKnown ? `${officeDownloadProgress}%` : '42%' }"
              ></div>
            </div>
            <div class="mt-2 text-xs text-blue-900">{{ officeProgressText }}</div>
            <button
              @click="toggleOfficeTransfer"
              :disabled="!isOfficeDownloading || !officeSupportsTransferControl"
              :class="[
                'mt-3 w-full rounded-lg py-2 text-sm font-medium',
                isOfficeDownloading && officeSupportsTransferControl
                  ? 'bg-blue-600 text-white hover:bg-blue-700 active:bg-blue-800'
                  : 'bg-blue-100 text-blue-300 cursor-not-allowed'
              ]"
            >
              {{ officeTransferButtonText }}
            </button>
          </div>
        </div>
      </div>

    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useLxPage } from '@lingxia/vue';
import '../../tailwind.css';

type PageData = {
  pdfUrl?: string;
  officeUrl?: string;
  officeFileType?: string;
  showMenu?: boolean;
  isPdfDownloading?: boolean;
  isOfficeDownloading?: boolean;
  officeDownloadState?: string;
  officeProgressKnown?: boolean;
  officeDownloadProgress?: number;
  officeProgressText?: string;
  officeSupportsTransferControl?: boolean;
  officeTransferButtonText?: string;
};

type PageActions = {
  onPdfUrlInput(event: any): void;
  onOfficeUrlInput(event: any): void;
  onOfficeFileTypeInput(event: any): void;
  toggleShowMenu(): void;
  openPdf(): void;
  openOffice(): void;
  toggleOfficeTransfer(): void;
};

const { data, actions } = useLxPage<PageData, PageActions>();
const {
  onPdfUrlInput,
  onOfficeUrlInput,
  onOfficeFileTypeInput,
  toggleShowMenu,
  openPdf,
  openOffice,
  toggleOfficeTransfer,
} = actions;

const pdfUrl = computed(() => data.pdfUrl || '');
const officeUrl = computed(() => data.officeUrl || '');
const officeFileType = computed(() => data.officeFileType || '');
const showMenu = computed(() => Boolean(data.showMenu));
const isPdfDownloading = computed(() => Boolean(data.isPdfDownloading));
const isOfficeDownloading = computed(() => Boolean(data.isOfficeDownloading));
const officeProgressKnown = computed(() => Boolean(data.officeProgressKnown));
const officeDownloadProgress = computed(() => data.officeDownloadProgress || 0);
const officeProgressText = computed(() => data.officeProgressText || 'Not started yet');
const officeSupportsTransferControl = computed(() => Boolean(data.officeSupportsTransferControl));
const officeTransferButtonText = computed(() => data.officeTransferButtonText || 'Pause Download');
const officePrimaryButtonText = computed(() => {
  if (data.officeDownloadState === 'paused') return 'Download Paused';
  if (isOfficeDownloading.value) return 'Downloading...';
  return 'Download and Open Document';
});
</script>
