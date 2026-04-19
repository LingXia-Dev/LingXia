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

      <template v-if="activeDemo === 'openFile'">
        <div class="bg-white rounded-lg shadow-sm">
          <div class="px-4 py-4 border-b border-gray-100">
            <div class="text-base text-gray-900 font-medium">PDF via lx.downloadFile()</div>
            <div class="text-xs text-gray-500 mt-1">Download to a temporary file with progress and pause/continue, then open with the native PDF viewer.</div>
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
              {{ pdfPrimaryButtonText }}
            </button>
            <div v-if="showPdfProgress" class="rounded-xl border border-blue-100 bg-blue-50/70 p-3">
              <div class="flex items-center justify-between text-xs text-blue-700">
                <span>PDF Transfer</span>
                <span>{{ pdfProgressKnown ? `${Math.round(pdfDownloadProgress)}%` : 'Streaming' }}</span>
              </div>
              <div v-if="pdfProgressKnown" class="mt-2 h-2 overflow-hidden rounded-full bg-blue-100">
                <div
                  class="h-full rounded-full bg-blue-500 transition-all duration-300"
                  :style="{ width: `${pdfDownloadProgress}%` }"
                ></div>
              </div>
              <div v-else class="mt-2 flex items-center gap-2 text-[11px] text-blue-700">
                <span class="inline-flex h-2.5 w-2.5 rounded-full bg-blue-500 animate-pulse"></span>
                <span>Waiting for precise progress from runtime…</span>
              </div>
              <div class="mt-2 text-xs text-blue-900">{{ pdfProgressText }}</div>
              <button
                @click="togglePdfTransfer"
                :disabled="!isPdfDownloading || !pdfSupportsTransferControl"
                :class="[
                  'mt-3 w-full rounded-lg py-2 text-sm font-medium',
                  isPdfDownloading && pdfSupportsTransferControl
                    ? 'bg-blue-600 text-white hover:bg-blue-700 active:bg-blue-800'
                    : 'bg-blue-100 text-blue-300 cursor-not-allowed'
                ]"
              >
                {{ pdfTransferButtonText }}
              </button>
            </div>
          </div>
        </div>

        <div class="bg-white rounded-lg shadow-sm">
          <div class="px-4 py-4 border-b border-gray-100">
            <div class="text-base text-gray-900 font-medium">Office via fetch()</div>
            <div class="text-xs text-gray-500 mt-1">Use web-standard fetch in page logic, save into usercache, then open with the native file API.</div>
          </div>
          <div class="px-4 py-4 space-y-3">
            <div>
              <div class="text-sm text-gray-600 mb-2">File URL:</div>
              <input
                type="text"
                :value="officeUrl"
                @input="onOfficeUrlInput({ detail: { value: ($event.target as HTMLInputElement).value } })"
                placeholder="Enter file URL"
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
              :disabled="isOfficeFetching"
              :class="[
                'w-full py-3 rounded-lg text-white font-medium',
                isOfficeFetching ? 'bg-gray-400 cursor-not-allowed' : 'bg-blue-500 hover:bg-blue-600 active:bg-blue-700'
              ]"
            >
              {{ officePrimaryButtonText }}
            </button>
            <div v-if="officeStatusText" class="rounded-xl border border-blue-100 bg-blue-50/70 p-3 text-xs text-blue-900">
              {{ officeStatusText }}
            </div>
          </div>
        </div>
      </template>

      <div v-else class="bg-white rounded-lg shadow-sm">
        <div class="px-4 py-4 border-b border-gray-100">
          <div class="text-base text-gray-900 font-medium">Choose File</div>
          <div class="text-xs text-gray-500 mt-1">Open the host chooser in a predefined folder instead of the system recent-files picker.</div>
        </div>
        <div class="px-4 py-4 space-y-3">
          <div class="text-sm text-gray-600">Default folder:</div>
          <div class="rounded-lg bg-gray-50 border border-gray-200 px-3 py-2 text-xs text-gray-700 break-all">
            {{ chooseFileDefaultPath }}
          </div>
          <button
            @click="chooseFileFromUserCache"
            class="w-full py-3 rounded-lg bg-blue-500 hover:bg-blue-600 active:bg-blue-700 text-white font-medium"
          >
            Open File Chooser
          </button>
          <div class="rounded-xl border border-gray-200 bg-gray-50 p-3 space-y-2">
            <div class="text-xs text-gray-500">Status</div>
            <div class="text-sm text-gray-900">{{ chooseFileStatusText }}</div>
            <div class="text-xs text-gray-500">Selected Path</div>
            <div class="text-xs text-gray-700 break-all">{{ chooseFileSelectedPath || 'None' }}</div>
            <div class="text-xs text-gray-500">Detected Type</div>
            <div class="text-xs text-gray-700">{{ chooseFileSelectedType || 'Unknown' }}</div>
          </div>
          <button
            @click="openChosenFile"
            :disabled="!chooseFileSelectedPath"
            :class="[
              'w-full py-3 rounded-lg text-white font-medium',
              chooseFileSelectedPath
                ? 'bg-gray-900 hover:bg-black active:bg-gray-800'
                : 'bg-gray-400 cursor-not-allowed'
            ]"
          >
            Open Selected File
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useLxPage } from '@lingxia/vue';
import '../../tailwind.css';

type ActiveDemo = 'openFile' | 'chooseFile';

type PageData = {
  activeDemo?: ActiveDemo;
  pdfUrl?: string;
  officeUrl?: string;
  officeFileType?: string;
  showMenu?: boolean;
  chooseFileDefaultPath?: string;
  chooseFileStatusText?: string;
  chooseFileSelectedPath?: string;
  chooseFileSelectedType?: string;
  isPdfDownloading?: boolean;
  pdfDownloadState?: string;
  pdfProgressKnown?: boolean;
  pdfDownloadProgress?: number;
  pdfProgressText?: string;
  pdfSupportsTransferControl?: boolean;
  pdfTransferButtonText?: string;
  isOfficeFetching?: boolean;
  officeStatusText?: string;
};

type PageActions = {
  onPdfUrlInput(event: any): void;
  onOfficeUrlInput(event: any): void;
  onOfficeFileTypeInput(event: any): void;
  toggleShowMenu(): void;
  chooseFileFromUserCache(): void;
  openChosenFile(): void;
  openPdf(): void;
  openOffice(): void;
  togglePdfTransfer(): void;
};

const { data, actions } = useLxPage<PageData, PageActions>();
const {
  onPdfUrlInput,
  onOfficeUrlInput,
  onOfficeFileTypeInput,
  toggleShowMenu,
  chooseFileFromUserCache,
  openChosenFile,
  openPdf,
  openOffice,
  togglePdfTransfer,
} = actions;

const activeDemo = computed<ActiveDemo>(() => data.activeDemo || 'openFile');
const pdfUrl = computed(() => data.pdfUrl || '');
const officeUrl = computed(() => data.officeUrl || '');
const officeFileType = computed(() => data.officeFileType || '');
const showMenu = computed(() => Boolean(data.showMenu));
const chooseFileDefaultPath = computed(() => data.chooseFileDefaultPath || '');
const chooseFileStatusText = computed(() => data.chooseFileStatusText || 'Choose a file');
const chooseFileSelectedPath = computed(() => data.chooseFileSelectedPath || '');
const chooseFileSelectedType = computed(() => data.chooseFileSelectedType || '');
const isPdfDownloading = computed(() => Boolean(data.isPdfDownloading));
const pdfDownloadState = computed(() => data.pdfDownloadState || 'idle');
const pdfProgressKnown = computed(() => Boolean(data.pdfProgressKnown));
const pdfDownloadProgress = computed(() => data.pdfDownloadProgress || 0);
const pdfProgressText = computed(() => data.pdfProgressText || '');
const pdfSupportsTransferControl = computed(() => Boolean(data.pdfSupportsTransferControl));
const pdfTransferButtonText = computed(() => data.pdfTransferButtonText || 'Pause Download');
const showPdfProgress = computed(() => pdfDownloadState.value !== 'idle' || !!data.pdfProgressText);
const pdfPrimaryButtonText = computed(() => {
  if (pdfDownloadState.value === 'paused') return 'Download Paused';
  if (pdfDownloadState.value === 'opening') return 'Opening File...';
  if (isPdfDownloading.value) return 'Downloading...';
  return 'Download and Preview PDF';
});
const isOfficeFetching = computed(() => Boolean(data.isOfficeFetching));
const officeStatusText = computed(() => data.officeStatusText || '');
const officePrimaryButtonText = computed(() => isOfficeFetching.value ? 'Fetching and Opening File...' : 'Fetch and Open File');
</script>
