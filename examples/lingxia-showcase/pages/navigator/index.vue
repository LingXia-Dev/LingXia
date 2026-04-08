<template>
  <div class="min-h-screen bg-gray-50">
    <div class="px-4 py-5 space-y-4">
      <!-- Header -->
      <div class="bg-gradient-to-br from-blue-500 via-blue-600 to-cyan-600 rounded-2xl px-5 py-6 shadow-lg">
        <div class="flex items-center gap-3 mb-2">
          <div class="w-10 h-10 bg-white/20 backdrop-blur-sm rounded-xl flex items-center justify-center">
            <svg viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2.5" class="w-6 h-6">
              <path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z" />
            </svg>
          </div>
          <div>
            <div class="text-xl text-white font-bold">LxNavigator</div>
            <div class="text-sm text-white/80">Declarative navigation component</div>
          </div>
        </div>
        <div class="text-xs text-white/70 mt-3 leading-relaxed">
          Navigate between pages, open external apps, and handle browser URLs with a simple declarative API
        </div>
      </div>

      <!-- In-App Navigation -->
      <div class="space-y-3">
        <div class="flex items-center gap-2 px-1">
          <div class="w-1 h-4 bg-blue-500 rounded-full" />
          <h2 class="text-base font-semibold text-gray-900">In-App Navigation</h2>
        </div>

        <div class="bg-white rounded-xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="p-4">
            <div class="text-xs text-gray-500 mb-3 font-medium uppercase tracking-wider">Methods</div>
            <div class="grid grid-cols-2 gap-3">
              <!-- Navigate -->
              <LxNavigator
                url="pages/device/index?type=device"
                open-type="navigate"
                @success="addLog('✓ Navigate to home')"
              >
                <div class="flex flex-col items-center justify-center py-4 px-2 bg-blue-50 hover:bg-blue-100 active:bg-blue-200 text-blue-700 rounded-xl transition-colors h-full">
                  <span class="text-lg mb-1">➡️</span>
                  <span class="text-sm font-medium">Navigate</span>
                  <span class="text-[10px] opacity-70">Push to stack</span>
                </div>
              </LxNavigator>

              <!-- Redirect -->
              <LxNavigator
                url="pages/device/index?type=device"
                open-type="redirect"
                @success="addLog('✓ Redirect to home')"
              >
                <div class="flex flex-col items-center justify-center py-4 px-2 bg-purple-50 hover:bg-purple-100 active:bg-purple-200 text-purple-700 rounded-xl transition-colors h-full">
                  <span class="text-lg mb-1">🔀</span>
                  <span class="text-sm font-medium">Redirect</span>
                  <span class="text-[10px] opacity-70">Replace current</span>
                </div>
              </LxNavigator>

              <!-- Navigate Back -->
              <LxNavigator
                open-type="navigateBack"
                :delta="1"
                @success="addLog('✓ Back 1 page')"
              >
                <div class="flex flex-col items-center justify-center py-4 px-2 bg-gray-100 hover:bg-gray-200 active:bg-gray-300 text-gray-700 rounded-xl transition-colors h-full">
                  <span class="text-lg mb-1">⬅️</span>
                  <span class="text-sm font-medium">Back</span>
                  <span class="text-[10px] opacity-70">Pop from stack</span>
                </div>
              </LxNavigator>

              <!-- ReLaunch -->
              <LxNavigator
                url="pages/device/index?type=screen"
                open-type="reLaunch"
                @success="addLog('✓ ReLaunch to home')"
              >
                <div class="flex flex-col items-center justify-center py-4 px-2 bg-orange-50 hover:bg-orange-100 active:bg-orange-200 text-orange-700 rounded-xl transition-colors h-full">
                  <span class="text-lg mb-1">🚀</span>
                  <span class="text-sm font-medium">ReLaunch</span>
                  <span class="text-[10px] opacity-70">Reset all</span>
                </div>
              </LxNavigator>
            </div>
          </div>

          <!-- Switch Tab -->
          <div class="p-4 border-t border-gray-100">
            <div class="flex items-start justify-between mb-3">
              <div>
                <div class="text-sm font-medium text-gray-900">Switch Tab</div>
                <div class="text-xs text-gray-500 mt-0.5">Navigate to tab bar page</div>
              </div>
            </div>
            <div class="grid grid-cols-3 gap-2">
              <LxNavigator
                url="pages/home/index"
                open-type="switchTab"
                @success="addLog('✓ Switch to Home tab')"
              >
                <div class="py-2 px-3 bg-blue-50 hover:bg-blue-100 text-blue-600 rounded-lg text-xs font-medium text-center transition-colors">
                  🏠 Home
                </div>
              </LxNavigator>
              <LxNavigator
                url="pages/API/index"
                open-type="switchTab"
                @success="addLog('✓ Switch to API tab')"
              >
                <div class="py-2 px-3 bg-purple-50 hover:bg-purple-100 text-purple-600 rounded-lg text-xs font-medium text-center transition-colors">
                  📡 API
                </div>
              </LxNavigator>
              <LxNavigator
                url="pages/todo/index"
                open-type="switchTab"
                @success="addLog('✓ Switch to Todo tab')"
              >
                <div class="py-2 px-3 bg-green-50 hover:bg-green-100 text-green-600 rounded-lg text-xs font-medium text-center transition-colors">
                  ✓ Todo
                </div>
              </LxNavigator>
            </div>
          </div>
        </div>
      </div>

      <!-- External Navigation -->
      <div class="space-y-3">
        <div class="flex items-center gap-2 px-1">
          <div class="w-1 h-4 bg-green-500 rounded-full" />
          <h2 class="text-base font-semibold text-gray-900">External Navigation</h2>
        </div>

        <div class="bg-white rounded-xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="p-4 space-y-3">
            <LxNavigator
              app-id="lingxia-chat"
              @success="addLog('✓ Opening other LxApp')"
              @fail="onFailWithMessage('Failed to open LxApp', $event)"
            >
              <div class="w-full py-2.5 px-4 bg-gradient-to-r from-green-500 to-emerald-500 hover:from-green-600 hover:to-emerald-600 text-white rounded-lg text-sm font-medium text-center transition-all shadow-sm">
                <div class="flex items-center justify-center gap-2">
                  <span>📱</span>
                  <span>Open Other LxApp</span>
                </div>
              </div>
            </LxNavigator>

            <LxNavigator
              url="https://www.deepseek.com"
              target="self"
              @success="addLog('✓ Opening DeepSeek in-app')"
              @fail="onFailWithMessage('Failed to open in-app browser', $event)"
            >
              <div class="w-full py-2.5 px-4 bg-gradient-to-r from-blue-600 to-cyan-600 hover:from-blue-700 hover:to-cyan-700 text-white rounded-lg text-sm font-medium text-center transition-all shadow-sm">
                <div class="flex items-center justify-center gap-2">
                  <span>🔗</span>
                  <span>Open DeepSeek</span>
                </div>
              </div>
            </LxNavigator>

            <LxNavigator
              url="https://www.deepseek.com"
              target="browser"
              @success="addLog('✓ Opening DeepSeek in external browser')"
              @fail="onFailWithMessage('Failed to open external browser', $event)"
            >
              <div class="w-full py-2.5 px-4 bg-gradient-to-r from-gray-600 to-gray-700 hover:from-gray-700 hover:to-gray-800 text-white rounded-lg text-sm font-medium text-center transition-all shadow-sm">
                <div class="flex items-center justify-center gap-2">
                  <span>🌐</span>
                  <span>Open DeepSeek in Default Browser</span>
                </div>
              </div>
            </LxNavigator>
          </div>
        </div>
      </div>

      <!-- Phone Call -->
      <div class="space-y-3">
        <div class="flex items-center gap-2 px-1">
          <div class="w-1 h-4 bg-rose-500 rounded-full" />
          <h2 class="text-base font-semibold text-gray-900">Phone Call</h2>
        </div>

        <div class="bg-white rounded-xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="p-4">
            <div class="flex items-start justify-between mb-3">
              <div>
                <div class="text-sm font-medium text-gray-900">Make Phone Call</div>
                <div class="text-xs text-gray-500 mt-0.5">Trigger system dialer with tel open-type</div>
              </div>
            </div>
            <LxNavigator
              open-type="tel"
              phone-number="10086"
              @success="addLog('✓ Making phone call')"
              @fail="onFailWithMessage('Failed to make call', $event)"
            >
              <div class="w-full py-3 px-4 bg-gradient-to-r from-rose-500 to-pink-500 hover:from-rose-600 hover:to-pink-600 active:from-rose-700 active:to-pink-700 text-white rounded-xl text-sm font-medium text-center transition-all shadow-sm">
                <div class="flex items-center justify-center gap-3">
                  <div class="w-8 h-8 bg-white/20 rounded-full flex items-center justify-center">
                    <svg viewBox="0 0 24 24" fill="currentColor" class="w-4 h-4">
                      <path d="M6.62 10.79c1.44 2.83 3.76 5.14 6.59 6.59l2.2-2.2c.27-.27.67-.36 1.02-.24 1.12.37 2.33.57 3.57.57.55 0 1 .45 1 1V20c0 .55-.45 1-1 1-9.39 0-17-7.61-17-17 0-.55.45-1 1-1h3.5c.55 0 1 .45 1 1 0 1.25.2 2.45.57 3.57.11.35.03.74-.25 1.02l-2.2 2.2z"/>
                    </svg>
                  </div>
                  <div class="flex flex-col items-start">
                    <span class="text-white/80 text-xs">Call</span>
                    <span class="text-base font-semibold tracking-wide">10086</span>
                  </div>
                </div>
              </div>
            </LxNavigator>
          </div>
        </div>
      </div>

      <!-- Event Logs -->
      <div class="bg-white rounded-xl shadow-sm border border-gray-100 overflow-hidden">
        <div class="p-4">
          <div class="text-xs text-gray-500 mb-3 font-medium uppercase tracking-wider">Event Logs</div>
          <div v-if="logs.length === 0" class="text-xs text-gray-400">No events yet</div>
          <div v-else class="space-y-2">
            <div
              v-for="(log, index) in logs"
              :key="`${log}-${index}`"
              class="text-xs text-gray-700 bg-gray-50 border border-gray-100 rounded-lg px-3 py-2 break-all"
            >
              {{ log }}
            </div>
          </div>
        </div>
      </div>

      <!-- Info Card -->
      <div class="bg-blue-50 border border-blue-100 rounded-xl p-4">
        <div class="flex gap-3">
          <div class="text-blue-500 flex-shrink-0 mt-0.5">
            <svg viewBox="0 0 24 24" fill="currentColor" class="w-5 h-5">
              <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z" />
            </svg>
          </div>
          <div class="flex-1">
            <div class="text-sm font-medium text-blue-900 mb-1">Smart & Simple</div>
            <div class="text-xs text-blue-700 leading-relaxed">
              • HTTPS URLs → auto open in browser<br />
              • appId → auto target other lxapp<br />
              • Pass data via query string in path
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { LxNavigator } from '@lingxia/vue';
import '../../tailwind.css';

const logs = ref<string[]>([]);

function addLog(message: string) {
  logs.value = [`[${new Date().toLocaleTimeString()}] ${message}`, ...logs.value].slice(0, 10);
}

function onFailWithMessage(label: string, event: any) {
  const errMsg = event?.detail?.errMsg || 'Unknown error';
  addLog(`✗ ${label}: ${errMsg}`);
}
</script>
