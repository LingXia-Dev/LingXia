<template>
  <div class="min-h-screen bg-gray-100 overflow-y-auto">
    <div class="px-3 py-2 pb-12 space-y-2">

      <!-- Header Card - Description -->
      <div class="bg-white rounded-lg shadow-sm">
        <div class="px-4 py-6 text-center">
          <div class="w-12 h-12 mx-auto mb-3 text-blue-500">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z"/>
            </svg>
          </div>
          <div class="text-base text-gray-900 font-medium">
            The following demonstrates the capabilities provided by LingXia.
          </div>
        </div>
      </div>

      <!-- Cloud - Dropdown -->
      <div class="bg-white rounded-lg shadow-sm">
        <div
          class="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50"
          @click="toggleSection({ section: 'cloud' })"
        >
          <div class="text-base text-gray-900">Cloud</div>
          <div class="w-6 h-6 text-gray-400">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <path d="M6 19a4 4 0 0 1-.4-7.98A5.5 5.5 0 0 1 16 8.5h.5a4.5 4.5 0 1 1 .5 9H6z"/>
            </svg>
          </div>
        </div>

        <div v-if="expandedSections.cloud" class="border-t border-gray-100 bg-gray-50">
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between"
            @click="navigateToTestMiniApp"
          >
            <div class="text-sm text-gray-700">Open Another LxApp</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToCloudPage({ type: 'auth' })"
          >
            <div class="text-sm text-gray-700">Cloud Auth Demo</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToCloudPage({ type: 'mqtt' })"
          >
            <div class="text-sm text-gray-700">Cloud MQTT Demo</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
        </div>
      </div>

      <!-- Navigation - Dropdown -->
      <div class="bg-white rounded-lg shadow-sm">
        <div
          class="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50"
          @click="toggleSection({ section: 'navigation' })"
        >
          <div class="text-base text-gray-900">Navigation</div>
          <div class="w-6 h-6 text-gray-400">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <path d="M13 7l5 5-5 5M6 7l5 5-5 5"/>
            </svg>
          </div>
        </div>

        <div v-if="expandedSections.navigation" class="border-t border-gray-100 bg-gray-50">
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between"
            @click="openDeepSeek"
          >
            <div class="text-sm text-gray-700">Open DeepSeek</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
        </div>
      </div>

      <!-- User Interface - Dropdown -->
      <div class="bg-white rounded-lg shadow-sm">
        <div
          class="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50"
          @click="toggleSection({ section: 'interface' })"
        >
          <div class="text-base text-gray-900">User Interface</div>
          <div class="w-6 h-6 text-gray-400">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>
              <polyline points="9,22 9,12 15,12 15,22"/>
            </svg>
          </div>
        </div>

        <div v-if="expandedSections.interface" class="border-t border-gray-100 bg-gray-50">
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between"
            @click="navigateToUIPage({ type: 'navigation' })"
          >
            <div class="text-sm text-gray-700">Page Navigation</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToUIPage({ type: 'toast' })"
          >
            <div class="text-sm text-gray-700">Toast</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToUIPage({ type: 'modal' })"
          >
            <div class="text-sm text-gray-700">Modal</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToUIPage({ type: 'navbar' })"
          >
            <div class="text-sm text-gray-700">NavigationBar</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToUIPage({ type: 'tabbar' })"
          >
            <div class="text-sm text-gray-700">TabBar</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToUIPage({ type: 'actionsheet' })"
          >
            <div class="text-sm text-gray-700">Action Sheet</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToUIPage({ type: 'popup' })"
          >
            <div class="text-sm text-gray-700">Popup</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToPullDownRefreshPage"
          >
            <div class="text-sm text-gray-700">Pull Down Refresh</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
        </div>
      </div>

      <!-- System - Dropdown -->
      <div class="bg-white rounded-lg shadow-sm">
        <div
          class="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50"
          @click="toggleSection({ section: 'system' })"
        >
          <div class="text-base text-gray-900">System</div>
          <div class="w-6 h-6 text-gray-400">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <path d="M4 4h16v16H4z"/>
            </svg>
          </div>
        </div>

        <div v-if="expandedSections.system" class="border-t border-gray-100 bg-gray-50">
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between"
            @click="navigateToSystemPage({ type: 'appBaseInfo' })"
          >
            <div class="text-sm text-gray-700">App Base Info</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToSystemPage({ type: 'systemSetting' })"
          >
            <div class="text-sm text-gray-700">System Setting</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
        </div>
      </div>

      <!-- Device - Dropdown -->
      <div class="bg-white rounded-lg shadow-sm">
        <div
          class="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50"
          @click="toggleSection({ section: 'device' })"
        >
          <div class="text-base text-gray-900">Device</div>
          <div class="w-6 h-6 text-gray-400">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <rect x="5" y="2" width="14" height="20" rx="2"/>
              <path d="M12 18h.01"/>
            </svg>
          </div>
        </div>

        <div v-if="expandedSections.device" class="border-t border-gray-100 bg-gray-50">
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between"
            @click="navigateToDevicePage({ type: 'device' })"
          >
            <div class="text-sm text-gray-700">Device Info</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToDevicePage({ type: 'screen' })"
          >
            <div class="text-sm text-gray-700">Screen Info</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToDevicePage({ type: 'vibrate' })"
          >
            <div class="text-sm text-gray-700">Vibration</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToDevicePage({ type: 'dial' })"
          >
            <div class="text-sm text-gray-700">Phone Call</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToDevicePage({ type: 'orientation' })"
          >
            <div class="text-sm text-gray-700">Device Orientation</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToWifiPage"
          >
            <div class="text-sm text-gray-700">WiFi</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
        </div>
      </div>

      <!-- Media - Dropdown -->
      <div class="bg-white rounded-lg shadow-sm">
        <div
          class="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50"
          @click="toggleSection({ section: 'media' })"
        >
          <div class="text-base text-gray-900">Media & Camera</div>
          <div class="w-6 h-6 text-gray-400">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <polygon points="5,3 19,12 5,21"/>
            </svg>
          </div>
        </div>

        <div v-if="expandedSections.media" class="border-t border-gray-100 bg-gray-50">
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between"
            @click="navigateToMediaPage({ type: 'Pictures' })"
          >
            <div class="text-sm text-gray-700">Pictures</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToMediaPage({ type: 'Videos' })"
          >
            <div class="text-sm text-gray-700">Videos</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToMediaPage({ type: 'scanCode' })"
          >
            <div class="text-sm text-gray-700">ScanCode</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToMediaPage({ type: 'imageInfo' })"
          >
            <div class="text-sm text-gray-700">Image Tools</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToMediaPage({ type: 'videoTools' })"
          >
            <div class="text-sm text-gray-700">Video Tools</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
          <div
            class="px-4 py-3 hover:bg-gray-100 cursor-pointer flex items-center justify-between border-t border-gray-200"
            @click="navigateToMediaPage({ type: 'saveToAlbum' })"
          >
            <div class="text-sm text-gray-700">Save to Album</div>
            <div class="w-4 h-4 text-gray-400">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </div>
          </div>
        </div>
      </div>

      <!-- Document - Clickable -->
      <div class="bg-white rounded-lg shadow-sm">
        <div
          class="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50"
          @click="navigateToDocumentPage"
        >
          <div class="text-base text-gray-900">Document</div>
          <div class="w-6 h-6 text-gray-400">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
              <polyline points="14,2 14,8 20,8"/>
            </svg>
          </div>
        </div>
      </div>

      <!-- Location - Clickable -->
      <div class="bg-white rounded-lg shadow-sm">
        <div
          class="px-4 py-4 flex items-center justify-between cursor-pointer hover:bg-gray-50"
          @click="navigateToLocationPage"
        >
          <div class="text-base text-gray-900">Location</div>
          <div class="w-6 h-6 text-gray-400">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <path d="M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z"/>
              <circle cx="12" cy="10" r="3"/>
            </svg>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useLingXia } from '@lingxia/core/vue';
import '../../tailwind.css';

type ExpandedSections = {
  interface: boolean;
  device: boolean;
  system: boolean;
  navigation: boolean;
  media: boolean;
  document: boolean;
};

type PageActions = {
  data: { expandedSections?: ExpandedSections };
  toggleSection(payload: { section: string }): void;
  navigateToUIPage(payload: { type: string }): void;
  navigateToDevicePage(payload: { type: string }): void;
  navigateToWifiPage(): void;
  navigateToSystemPage(payload: { type: string }): void;
  navigateToLocationPage(): void;
  navigateToMediaPage(payload: { type: string }): void;
  navigateToDocumentPage(): void;
  navigateToTestMiniApp(): void;
  navigateToCloudPage(payload: { type: string }): void;
  openDeepSeek(): void;
  navigateToPullDownRefreshPage(): void;
};

const {
  data,
  toggleSection,
  navigateToUIPage,
  navigateToDevicePage,
  navigateToWifiPage,
  navigateToSystemPage,
  navigateToLocationPage,
  navigateToMediaPage,
  navigateToDocumentPage,
  navigateToTestMiniApp,
  openDeepSeek,
  navigateToPullDownRefreshPage,
} = useLingXia() as PageActions;

const expandedSections = computed(() => data.expandedSections ?? {
  interface: false,
  device: false,
  system: false,
  navigation: false,
  media: false,
  document: false,
});
</script>
