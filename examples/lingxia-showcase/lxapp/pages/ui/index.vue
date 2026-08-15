<template>
  <div class="h-screen bg-linear-to-br from-surface-50 to-surface-100 flex flex-col overflow-y-auto">
    <div class="flex-1 overflow-y-auto">
      <div class="pb-6 px-4 pt-6">
        <div v-if="chromeError" class="mb-4 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:text-red-400">
          {{ chromeError }}
        </div>

        <!-- Navigation Demo -->
        <template v-if="currentType === 'navigation'">
          <div class="mb-4 text-sm text-gray-600 font-semibold">navigateTo/Back, redirectTo</div>
          <div class="mb-5 bg-surface rounded-2xl shadow-sm border border-line-100 overflow-hidden">
            <div data-testid="ui-navigate-to" class="flex items-center justify-between px-5 py-4 hover:bg-surface-50 cursor-pointer border-b border-line-100" @click="demoNavigateTo">
              <div>
                <div class="text-sm text-gray-800 font-medium">Push this page again</div>
                <div class="text-xs text-gray-500 mt-0.5">navigateTo → this route; every entry is a fresh instance, watch the stack grow</div>
              </div>
              <span class="text-gray-400 text-lg">›</span>
            </div>
            <div data-testid="ui-navigate-back" class="flex items-center justify-between px-5 py-4 hover:bg-surface-50 cursor-pointer border-b border-line-100" @click="demoNavigateBack">
              <div class="text-sm text-gray-800 font-medium">Back to previous page</div>
              <span class="text-gray-400 text-lg">›</span>
            </div>
            <div data-testid="ui-redirect-to" class="flex items-center justify-between px-5 py-4 hover:bg-surface-50 cursor-pointer border-b border-line-100" @click="demoRedirectTo">
              <div class="text-sm text-gray-800 font-medium">Replace this page (redirectTo)</div>
              <span class="text-gray-400 text-lg">›</span>
            </div>
            <div data-testid="ui-switch-tab" class="flex items-center justify-between px-5 py-4 hover:bg-surface-50 cursor-pointer" @click="demoSwitchTab">
              <div class="text-sm text-gray-800 font-medium">Jump to Tab page</div>
              <span class="text-gray-400 text-lg">›</span>
            </div>
          </div>

          <!-- Page stack (same panel as the Page Stack & Reset demo) -->
          <div class="mb-5 bg-surface rounded-xl shadow-sm border border-line-100 overflow-hidden">
            <div class="px-4 py-3 border-b border-line-100 flex items-center justify-between">
              <div>
                <h3 class="text-base font-medium text-gray-900">Page stack</h3>
                <p class="text-xs text-gray-500 mt-0.5">getCurrentPages(), top is where you are</p>
              </div>
              <span
                data-testid="ui-page-stack-depth"
                class="text-xs font-mono font-medium text-violet-600 dark:text-violet-400 bg-violet-50 dark:bg-violet-950 rounded-full px-2.5 py-1"
              >
                {{ pageStack.length }}/10
              </span>
            </div>
            <div class="p-3 space-y-1.5" data-testid="ui-page-stack-list">
              <div
                v-for="entry in reversedPageStack"
                :key="entry.index"
                :class="[
                  'flex items-center gap-3 rounded-lg px-3 py-2 text-xs font-mono',
                  entry.current
                    ? 'bg-violet-50 dark:bg-violet-950 text-violet-700 dark:text-violet-300 border border-violet-200 dark:border-violet-800'
                    : 'bg-surface-50 text-gray-600 border border-line-100',
                ]"
              >
                <span class="w-5 text-right opacity-60">{{ entry.index + 1 }}</span>
                <span class="flex-1 truncate">{{ entry.name }}</span>
                <span v-if="entry.current" class="text-[10px] font-sans font-medium">you are here</span>
              </div>
              <div v-if="pageStack.length === 0" class="text-xs text-gray-400 text-center py-4">No page stack available</div>
            </div>
          </div>

          <!-- Instance identity -->
          <div class="mb-5 bg-surface rounded-xl shadow-sm border border-line-100 overflow-hidden">
            <div class="px-4 py-3 border-b border-line-100">
              <h3 class="text-base font-medium text-gray-900">Logic instance</h3>
              <p class="text-xs text-gray-500 mt-0.5">
                Push this page again and a new tag appears — every entry is a new Page instance
              </p>
            </div>
            <div class="px-4 py-4 space-y-3">
              <div class="flex items-center justify-between">
                <span class="text-sm text-gray-600">This instance</span>
                <span data-testid="lifecycle-instance-tag" class="text-sm font-mono font-medium text-violet-600 dark:text-violet-400">
                  #{{ instanceTag || '…' }}
                </span>
              </div>
              <div class="flex items-center justify-between">
                <span class="text-sm text-gray-600">Previous instance</span>
                <span data-testid="lifecycle-previous-tag" class="text-sm font-mono text-gray-500">
                  #{{ previousInstanceTag || '…' }}
                </span>
              </div>
              <div class="text-[11px] text-gray-400 leading-relaxed pt-1">
                The previous tag is read back from <code>lx.getStorage()</code>, which is
                app-scoped and survives the reset.
              </div>
            </div>
          </div>

          <!-- State that resets -->
          <div class="mb-5 bg-surface rounded-xl shadow-sm border border-line-100 overflow-hidden">
            <div class="px-4 py-3 border-b border-line-100">
              <h3 class="text-base font-medium text-gray-900">State that resets</h3>
              <p class="text-xs text-gray-500 mt-0.5">Dirty all three, leave, come back — everything is fresh</p>
            </div>
            <div class="p-4 grid grid-cols-2 gap-3">
              <button
                data-testid="lifecycle-bump-logic"
                @click="bumpLogicCounter"
                class="flex flex-col items-center justify-center py-4 px-2 bg-violet-50 hover:bg-violet-100 active:bg-violet-200 text-violet-700 dark:text-violet-400 rounded-xl transition-colors"
              >
                <span class="text-2xl font-bold" data-testid="lifecycle-logic-counter">{{ logicCounter }}</span>
                <span class="text-sm font-medium mt-1">Logic +1</span>
                <span class="text-[10px] opacity-70">this.setData</span>
              </button>
              <button
                data-testid="lifecycle-bump-view"
                @click="viewCounter += 1"
                class="flex flex-col items-center justify-center py-4 px-2 bg-cyan-50 hover:bg-cyan-100 active:bg-cyan-200 text-cyan-700 dark:text-cyan-400 rounded-xl transition-colors"
              >
                <span class="text-2xl font-bold" data-testid="lifecycle-view-counter">{{ viewCounter }}</span>
                <span class="text-sm font-medium mt-1">View +1</span>
                <span class="text-[10px] opacity-70">ref()</span>
              </button>
            </div>
            <div class="px-4 pb-4">
              <button
                data-testid="lifecycle-open-popup"
                @click="popupOpen = true"
                class="w-full py-2.5 px-4 bg-linear-to-r from-violet-500 to-fuchsia-500 hover:from-violet-600 hover:to-fuchsia-600 text-white rounded-lg text-sm font-medium transition-all shadow-sm"
              >
                Open H5 popup
              </button>
            </div>
          </div>

          <!-- Lifecycle log -->
          <div class="mb-5 bg-surface rounded-xl shadow-sm border border-line-100 overflow-hidden">
            <div class="px-4 py-3 border-b border-line-100">
              <h3 class="text-base font-medium text-gray-900">Lifecycle of this instance</h3>
              <p class="text-xs text-gray-500 mt-0.5">
                Stored in <code>data</code>, so it starts over with every entry
              </p>
            </div>
            <div class="p-4">
              <div v-if="events.length === 0" class="text-xs text-gray-400">No events yet</div>
              <div v-else class="space-y-2" data-testid="lifecycle-events">
                <div
                  v-for="(event, index) in events"
                  :key="`${event}-${index}`"
                  class="text-xs font-mono text-gray-700 bg-surface-50 border border-line-100 rounded-lg px-3 py-2"
                >
                  {{ event }}
                </div>
              </div>
              <div class="text-[11px] text-gray-400 leading-relaxed mt-3">
                Backgrounding the app fires <code>onHide</code> / <code>onShow</code> on the same
                instance. Only leaving the page fires <code>onUnload</code> and ends it.
              </div>
            </div>
          </div>

          <!-- A plain H5 overlay — the kind that used to still be open on re-entry -->
          <div
            v-if="popupOpen"
            data-testid="lifecycle-popup"
            class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 px-8"
            @click="popupOpen = false"
          >
            <div
              class="bg-surface rounded-2xl shadow-xl px-5 py-6 w-full max-w-xs text-center"
              @click.stop
            >
              <div class="text-base font-semibold text-gray-900">Pure H5 popup</div>
              <div class="text-xs text-gray-500 mt-2 leading-relaxed">
                Leave the page with this open. When you come back it is gone, because the
                document was reloaded behind the scenes.
              </div>
              <button
                class="mt-4 w-full py-2 rounded-lg bg-surface-100 hover:bg-surface-200 text-sm text-gray-700"
                @click="popupOpen = false"
              >
                Close
              </button>
            </div>
          </div>
        </template>

        <!-- Surface Demo -->
        <template v-if="currentType === 'surface'">
          <div class="mt-4 mb-6 px-4 text-center">
            <h1 class="text-2xl font-light text-gray-800 mb-2">lx.surface</h1>
            <div class="w-16 h-0.5 bg-surface-400 mx-auto"></div>
          </div>
          <div class="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
            <div class="px-4 py-4 space-y-4">
              <div class="space-y-3">
                <!-- Pick the surface kind first; the relevant placement control
                     (edge / position) appears for that kind. -->
                <div>
                  <div class="text-xs uppercase text-gray-500 tracking-wide mb-2">Kind</div>
                  <div class="grid grid-cols-3 gap-2">
                    <button v-for="kind in surfaceKinds" :key="kind.id" type="button" :disabled="surfaceActive"
                      :class="['py-2 text-sm rounded-lg transition-colors border disabled:opacity-50 disabled:cursor-not-allowed', surfaceKind === kind.id ? 'bg-surface-800 border-line-800 text-white' : 'bg-surface border-line-200 text-gray-600 hover:bg-surface-50']"
                      @click="surfaceKind = kind.id">
                      {{ kind.label }}
                    </button>
                  </div>
                  <div class="mt-2 text-xs text-gray-500 leading-5 bg-surface-50 rounded-lg px-3 py-2">
                    {{ surfaceKinds.find((k) => k.id === surfaceKind)?.hint }}
                  </div>
                </div>
                <div v-if="surfaceKind === 'aside'">
                  <div class="text-xs uppercase text-gray-500 tracking-wide mb-2">Edge</div>
                  <!-- Which side the aside docks to. -->
                  <div class="grid grid-cols-2 gap-2">
                    <button v-for="edge in surfaceEdges" :key="edge" type="button"
                      :class="['py-2 text-sm rounded-lg transition-colors border', surfaceEdge === edge ? 'bg-blue-500 border-blue-500 text-white' : 'bg-surface border-line-200 text-gray-600 hover:bg-surface-50']"
                      @click="surfaceEdge = edge">
                      {{ edge.charAt(0).toUpperCase() + edge.slice(1) }}
                    </button>
                  </div>
                </div>
                <div v-if="surfaceKind === 'float'">
                  <div class="text-xs uppercase text-gray-500 tracking-wide mb-2">Position</div>
                  <!-- Where the float popup sits above the main. -->
                  <div class="grid grid-cols-2 gap-2">
                    <button v-for="position in surfaceFloatPositions" :key="position" type="button"
                      :class="['py-2 text-sm rounded-lg transition-colors border', surfaceFloatPosition === position ? 'bg-indigo-500 border-indigo-500 text-white' : 'bg-surface border-line-200 text-gray-600 hover:bg-surface-50']"
                      @click="surfaceFloatPosition = position">
                      {{ position.charAt(0).toUpperCase() + position.slice(1) }}
                    </button>
                  </div>
                </div>
                <div>
                  <div class="text-xs uppercase text-gray-500 tracking-wide mb-2">Size (optional — px or %)</div>
                  <!-- Preferred-size hint; the Host may clamp/override it. A
                       percentage (e.g. 80%) is relative to the container; a bare
                       number is absolute px. -->
                  <div class="grid grid-cols-2 gap-2">
                    <input type="text" inputmode="text" placeholder="width (px or %)" v-model="surfaceWidth" @input="sizeError = ''"
                      class="py-2 px-3 text-sm rounded-lg border border-line-200 focus:outline-hidden focus:ring-2 focus:ring-surface-400" />
                    <input type="text" inputmode="text" placeholder="height (px or %)" v-model="surfaceHeight" @input="sizeError = ''"
                      class="py-2 px-3 text-sm rounded-lg border border-line-200 focus:outline-hidden focus:ring-2 focus:ring-surface-400" />
                  </div>
                  <p v-if="sizeError" data-testid="size-error" class="mt-2 text-xs text-rose-600 dark:text-rose-400">{{ sizeError }}</p>
                </div>
              </div>
              <button type="button" data-testid="open-surface" :data-surface-width="surfaceWidth"
                :data-surface-height="surfaceHeight" :disabled="surfaceActive"
                @click="handleOpenSurface"
                class="w-full bg-surface-800 hover:bg-surface-900 disabled:bg-surface-300 disabled:cursor-not-allowed text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors">
                {{ surfaceActive ? `Open ${surfaceKind} (already open)` : `Open ${surfaceKind}` }}
              </button>
              <p class="text-xs text-gray-500">
                Edge / position is applied when the surface opens. Changing it
                while a surface is open — or across hide → show — has no effect;
                close and re-open to apply a new value.
              </p>
              <div v-if="surfaceActive" class="grid grid-cols-3 gap-2">
                <button type="button" :disabled="surfaceVisible" @click="showActiveSurface()"
                  class="bg-emerald-500 hover:bg-emerald-600 disabled:bg-surface-200 disabled:text-gray-500 text-white py-2 px-3 rounded-lg text-sm font-medium transition-colors">
                  Show
                </button>
                <button type="button" :disabled="!surfaceVisible" @click="hideActiveSurface()"
                  class="bg-amber-500 hover:bg-amber-600 disabled:bg-surface-200 disabled:text-gray-500 text-white py-2 px-3 rounded-lg text-sm font-medium transition-colors">
                  Hide
                </button>
                <button type="button" @click="closeActiveSurface()"
                  class="bg-rose-500 hover:bg-rose-600 text-white py-2 px-3 rounded-lg text-sm font-medium transition-colors">
                  Close
                </button>
              </div>
              <div class="text-xs text-gray-500 uppercase tracking-wide">Surface status</div>
              <div class="text-sm text-gray-800 bg-surface-50 rounded-lg px-3 py-2 font-mono break-words">
                {{ surfaceMessage || 'No message received yet.' }}
              </div>
            </div>
          </div>
        </template>

        <!-- Toast Demo -->
        <template v-if="currentType === 'toast'">
          <div class="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">Toast Parameters</div>
          <div class="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
            <div class="px-3 py-3 space-y-3">
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Title</label>
                <input type="text" v-model="toastTitle" class="w-full px-3 py-2 border border-line-300 rounded-md focus:outline-hidden focus:ring-2 focus:ring-blue-500" placeholder="Enter toast title" />
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Icon</label>
                <button type="button" @click="chooseToastIcon" class="w-full px-3 py-2 border border-line-300 rounded-md flex items-center justify-between text-left text-gray-800">
                  <span>{{ toastIconDisplay }}</span>
                  <span class="text-xs text-blue-500 dark:text-blue-400">Change</span>
                </button>
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Duration (ms)</label>
                <input type="number" v-model.number="toastDuration" class="w-full px-3 py-2 border border-line-300 rounded-md focus:outline-hidden focus:ring-2 focus:ring-blue-500" min="500" max="10000" step="500" />
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Position</label>
                <button type="button" @click="chooseToastPosition" class="w-full px-3 py-2 border border-line-300 rounded-md flex items-center justify-between text-left text-gray-800">
                  <span>{{ toastPositionDisplay }}</span>
                  <span class="text-xs text-blue-500 dark:text-blue-400">Change</span>
                </button>
              </div>
              <div class="flex items-center">
                <input type="checkbox" id="toastMask" v-model="toastMask" class="h-4 w-4 text-blue-600 dark:text-blue-400 border-line-300 rounded" />
                <label for="toastMask" class="ml-2 block text-sm text-gray-700">Show mask</label>
              </div>
            </div>
          </div>
          <div class="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
            <div class="flex items-center justify-center px-4 py-4 hover:bg-surface-50 cursor-pointer border-b border-line-100"
              @click="showToastWithParams({ title: toastTitle, icon: toastIcon, duration: toastDuration, position: toastPosition, mask: toastMask })">
              <div class="text-base text-blue-600 dark:text-blue-400 font-medium">Show Toast</div>
            </div>
            <div class="flex items-center justify-center px-4 py-4 hover:bg-surface-50 cursor-pointer" @click="hideToast">
              <div class="text-base text-red-600 dark:text-red-400 font-medium">Hide Toast</div>
            </div>
          </div>
        </template>

        <!-- ActionSheet Demo -->
        <template v-if="currentType === 'actionsheet'">
          <div class="mx-1 mt-8 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
            <div class="px-4 py-10 text-base text-blue-600 dark:text-blue-400 font-medium text-center cursor-pointer hover:bg-blue-50" @click="showDemoActionSheet">
              Show Action Sheet
            </div>
          </div>
        </template>

        <!-- Modal Demo -->
        <template v-if="currentType === 'modal'">
          <div class="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">Modal Parameters</div>
          <div class="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
            <div class="px-3 py-3 space-y-3">
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Title</label>
                <input type="text" v-model="modalTitle" class="w-full px-3 py-2 border border-line-300 rounded-md focus:outline-hidden focus:ring-2 focus:ring-blue-500" />
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Content</label>
                <textarea v-model="modalContent" class="w-full px-3 py-2 border border-line-300 rounded-md focus:outline-hidden focus:ring-2 focus:ring-blue-500" rows="3" />
              </div>
              <div class="flex items-center">
                <input type="checkbox" id="modalShowCancel" v-model="modalShowCancel" class="h-4 w-4 text-blue-600 dark:text-blue-400 border-line-300 rounded" />
                <label for="modalShowCancel" class="ml-2 block text-sm text-gray-700">Show cancel button</label>
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Cancel Text</label>
                <input type="text" v-model="modalCancelText" class="w-full px-3 py-2 border border-line-300 rounded-md focus:outline-hidden focus:ring-2 focus:ring-blue-500" />
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Confirm Text</label>
                <input type="text" v-model="modalConfirmText" class="w-full px-3 py-2 border border-line-300 rounded-md focus:outline-hidden focus:ring-2 focus:ring-blue-500" />
              </div>
            </div>
          </div>
          <div class="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
            <div class="flex items-center justify-center px-4 py-4 hover:bg-surface-50 cursor-pointer"
              @click="showModalWithParams({ title: modalTitle, content: modalContent, showCancel: modalShowCancel, cancelText: modalCancelText, confirmText: modalConfirmText })">
              <div class="text-base text-blue-600 dark:text-blue-400 font-medium">Show Modal</div>
            </div>
          </div>
          <div v-if="modalResult" class="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
            <div class="px-3 py-3">
              <div class="text-sm font-medium text-gray-700 mb-3">Modal Result</div>
              <div class="bg-surface-50 rounded-lg p-3">
                <pre class="text-xs text-gray-600 whitespace-pre-wrap">{{ JSON.stringify(modalResult, null, 2) }}</pre>
              </div>
              <div class="mt-3 text-center text-sm text-red-600 dark:text-red-400 cursor-pointer hover:text-red-800 dark:hover:text-red-400" @click="clearModalResult">Clear Result</div>
            </div>
          </div>
        </template>

        <!-- NavigationBar Demo -->
        <template v-if="currentType === 'navbar'">
          <div class="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">NavigationBar APIs</div>
          <div class="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
            <div class="p-4 space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">Title</label>
                <div class="flex space-x-2">
                  <input type="text" v-model="navbarTitle" placeholder="Enter title" class="flex-1 px-2 py-1.5 text-sm border border-line-300 rounded focus:outline-hidden focus:ring-1 focus:ring-blue-500" />
                  <button @click="updateNavigationBarTitle({ title: navbarTitle })" class="px-3 py-1.5 text-sm bg-blue-500 text-white rounded hover:bg-blue-600">Set</button>
                </div>
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">Colors</label>
                <div class="space-y-2">
                  <div class="grid grid-cols-2 gap-2">
                    <input type="text" v-model="navbarBgColor" placeholder="Background #ffffff" class="px-2 py-1.5 text-sm border border-line-300 rounded" />
                    <input type="text" v-model="navbarTextColor" placeholder="Text #000000" class="px-2 py-1.5 text-sm border border-line-300 rounded" />
                  </div>
                  <button @click="updateNavigationBarColors({ backgroundColor: navbarBgColor || '#ffffff', frontColor: navbarTextColor || '#000000' })"
                    class="w-full px-3 py-1.5 text-sm bg-green-500 text-white rounded hover:bg-green-600">Set Colors</button>
                </div>
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">Presets</label>
                <div class="grid grid-cols-2 gap-1.5">
                  <button @click="updateNavigationBarTitle({ title: 'Dark Theme' }); updateNavigationBarColors({ backgroundColor: '#1f2937', frontColor: '#ffffff' })"
                    class="px-2 py-1.5 bg-surface-800 text-white rounded hover:bg-surface-900 text-xs">Dark</button>
                  <button @click="updateNavigationBarTitle({ title: 'Blue Theme' }); updateNavigationBarColors({ backgroundColor: '#3b82f6', frontColor: '#ffffff' })"
                    class="px-2 py-1.5 bg-blue-500 text-white rounded hover:bg-blue-600 text-xs">Blue</button>
                  <button @click="updateNavigationBarTitle({ title: 'Light Theme' }); updateNavigationBarColors({ backgroundColor: '#ffffff', frontColor: '#000000' })"
                    class="px-2 py-1.5 bg-surface text-foreground border border-line-300 rounded hover:bg-surface-50 text-xs">Light</button>
                  <button @click="updateNavigationBarTitle({ title: 'Green Theme' }); updateNavigationBarColors({ backgroundColor: '#10b981', frontColor: '#ffffff' })"
                    class="px-2 py-1.5 bg-green-500 text-white rounded hover:bg-green-600 text-xs">Green</button>
                </div>
              </div>
            </div>
          </div>
        </template>

        <!-- Appearance Demo -->
        <template v-if="currentType === 'appearance'">
          <div class="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">Appearance APIs</div>
          <div class="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
            <div class="px-4 py-3 border-b border-line-100">
              <h3 class="text-base font-medium text-gray-900">Light / Dark</h3>
              <p class="text-sm text-gray-500 mt-1">
                <code class="text-xs">lx.appearance.set()</code> picks this lxapp&apos;s branch
                independently of the host shell; the preference persists per lxapp.
              </p>
            </div>
            <div class="p-4 space-y-4">
              <div class="grid grid-cols-3 gap-1.5" data-testid="ui-appearance">
                <button
                  v-for="option in APPEARANCE_OPTIONS"
                  :key="option"
                  type="button"
                  :data-testid="`ui-appearance-${option}`"
                  :data-selected="appearance.preference === option"
                  @click="setAppearance({ preference: option })"
                  class="px-2 py-1.5 rounded text-xs font-medium capitalize border transition-colors"
                  :class="appearance.preference === option
                    ? 'bg-blue-500 text-white border-blue-500'
                    : 'bg-surface text-gray-700 border-line-300 hover:bg-surface-50'"
                >
                  {{ option }}
                </button>
              </div>

              <div class="rounded-lg bg-surface-50 border border-line-100 divide-y divide-line-100">
                <div class="flex items-center justify-between px-3 py-2">
                  <span class="text-sm text-gray-500">Preference</span>
                  <span class="text-sm font-medium text-gray-900" data-testid="ui-appearance-preference">{{ appearance.preference }}</span>
                </div>
                <div class="flex items-center justify-between px-3 py-2">
                  <span class="text-sm text-gray-500">Resolved</span>
                  <span class="text-sm font-medium text-gray-900" data-testid="ui-appearance-resolved">{{ appearance.resolved }}</span>
                </div>
              </div>

              <p class="text-xs text-gray-400 leading-relaxed">
                The runtime projects the resolved branch into every page as <code>color-scheme</code>
                plus <code>data-theme</code> on <code>&lt;html&gt;</code>. This app&apos;s Tailwind palette
                is bound to CSS variables keyed off that attribute, so pages need no per-element dark
                variants.
              </p>
            </div>
          </div>
        </template>

        <!-- TabBar Demo -->
        <template v-if="currentType === 'tabbar'">
          <div class="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">TabBar APIs</div>

          <!-- Visibility Controls -->
          <div class="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
            <div class="px-4 py-3 border-b border-line-100">
              <h3 class="text-base font-medium text-gray-900">Visibility Controls</h3>
            </div>
            <div class="p-4 space-y-4">
              <div class="flex space-x-3">
                <button @click="revealTabBar()" class="flex-1 bg-green-500 hover:bg-green-600 text-white py-2 px-4 rounded-lg text-sm font-medium">Show TabBar</button>
                <button @click="concealTabBar()" class="flex-1 bg-red-500 hover:bg-red-600 text-white py-2 px-4 rounded-lg text-sm font-medium">Hide TabBar</button>
              </div>
              <div class="pt-2 border-t border-line-100">
                <label class="block text-sm font-medium text-gray-700 mb-2">Update Tab 1 Text</label>
                <div class="flex space-x-2">
                  <input type="text" v-model="itemText" class="flex-1 px-3 py-2 border border-line-300 rounded-lg" placeholder="Enter new text" />
                  <button @click="updateTabBarItem({ index: 1, text: itemText })" class="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600">Update</button>
                </div>
              </div>
            </div>
          </div>

          <!-- Red Dot Controls -->
          <div class="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
            <div class="px-4 py-3 border-b border-line-100">
              <h3 class="text-base font-medium text-gray-900">Red Dot Controls</h3>
            </div>
            <div class="p-4">
              <div class="flex space-x-3">
                <button @click="enableTabBarRedDot({ index: 1 })" class="flex-1 bg-red-500 hover:bg-red-600 text-white py-2 px-4 rounded-lg text-sm font-medium">Show Red Dot</button>
                <button @click="disableTabBarRedDot({ index: 1 })" class="flex-1 bg-surface-500 hover:bg-surface-600 text-white py-2 px-4 rounded-lg text-sm font-medium">Hide Red Dot</button>
              </div>
            </div>
          </div>

          <!-- Badge Controls -->
          <div class="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
            <div class="px-4 py-3 border-b border-line-100">
              <h3 class="text-base font-medium text-gray-900">Badge Controls</h3>
            </div>
            <div class="p-4 space-y-3">
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">Badge Text</label>
                <input type="text" v-model="badgeText" class="w-full px-3 py-2 border border-line-300 rounded-lg text-sm" placeholder="Enter badge text" />
              </div>
              <div class="flex space-x-3">
                <button @click="updateTabBarBadge({ index: 1, text: badgeText })" class="flex-1 bg-orange-500 hover:bg-orange-600 text-white py-2 px-4 rounded-lg text-sm font-medium">Set Badge</button>
                <button @click="clearTabBarBadge({ index: 1 })" class="flex-1 bg-surface-500 hover:bg-surface-600 text-white py-2 px-4 rounded-lg text-sm font-medium">Remove Badge</button>
              </div>
            </div>
          </div>

          <!-- Style Controls -->
          <div class="mx-1 mb-4 bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden">
            <div class="px-4 py-3 border-b border-line-100">
              <h3 class="text-base font-medium text-gray-900">Style Controls</h3>
            </div>
            <div class="p-4 space-y-3">
              <div class="grid grid-cols-2 gap-3">
                <div>
                  <label class="block text-sm font-medium text-gray-700 mb-1">Text Color</label>
                  <div class="flex items-center space-x-2">
                    <div class="w-8 h-8 border border-line-300 rounded" :style="{ backgroundColor: tabColor }"></div>
                    <input type="text" v-model="tabColor" class="flex-1 px-2 py-1 border border-line-300 rounded text-sm" />
                  </div>
                </div>
                <div>
                  <label class="block text-sm font-medium text-gray-700 mb-1">Selected Color</label>
                  <div class="flex items-center space-x-2">
                    <div class="w-8 h-8 border border-line-300 rounded" :style="{ backgroundColor: tabSelectedColor }"></div>
                    <input type="text" v-model="tabSelectedColor" class="flex-1 px-2 py-1 border border-line-300 rounded text-sm" />
                  </div>
                </div>
              </div>
              <button @click="updateTabBarForegrounds({ color: tabColor, selectedColor: tabSelectedColor })"
                class="w-full bg-blue-500 hover:bg-blue-600 text-white py-2 px-4 rounded-lg text-sm font-medium">Apply Custom Style</button>
              <div class="mt-4">
                <label class="block text-sm font-medium text-gray-700 mb-2">Preset Themes</label>
                <div class="grid grid-cols-2 gap-2">
                  <button @click="applyTheme({ color: '#666666', selectedColor: '#007AFF' })"
                    class="px-3 py-2 bg-surface-100 hover:bg-surface-200 text-gray-700 rounded-lg text-sm font-medium">Default</button>
                  <button @click="applyTheme({ color: '#CCCCCC', selectedColor: '#0A84FF' })"
                    class="px-3 py-2 bg-surface-800 hover:bg-surface-900 text-white rounded-lg text-sm font-medium">Dark</button>
                  <button @click="applyTheme({ color: '#8E8E93', selectedColor: '#34C759' })"
                    class="px-3 py-2 bg-green-100 hover:bg-green-200 text-green-700 dark:text-green-400 rounded-lg text-sm font-medium">Green</button>
                  <button @click="applyTheme({ color: '#8E8E93', selectedColor: '#AF52DE' })"
                    class="px-3 py-2 bg-purple-100 hover:bg-purple-200 text-purple-700 dark:text-purple-400 rounded-lg text-sm font-medium">Purple</button>
                </div>
              </div>
            </div>
          </div>
        </template>

      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useLxPage } from '@lingxia/vue';
import '../../tailwind.css';

const {
  data, actions,
} = useLxPage();
const {
  demoNavigateTo,
  demoNavigateBack,
  demoSwitchTab,
  demoRedirectTo,
  bumpLogicCounter,
  showToastWithParams,
  hideToast,
  showModalWithParams,
  clearModalResult,
  updateNavigationBarTitle,
  updateNavigationBarColors,
  enableTabBarRedDot,
  disableTabBarRedDot,
  updateTabBarBadge,
  clearTabBarBadge,
  revealTabBar,
  concealTabBar,
  updateTabBarForegrounds,
  updateTabBarItem,
  setAppearance,
  chooseToastIcon,
  chooseToastPosition,
  showDemoActionSheet,
  openSurfaceDemo,
  showActiveSurface,
  hideActiveSurface,
  closeActiveSurface,
} = actions;

const currentType = computed(() => data.currentType ?? 'navigation');
const pageStack = computed(() => data.pageStack ?? []);
const instanceTag = computed(() => data.instanceTag ?? '');
const previousInstanceTag = computed(() => data.previousInstanceTag ?? '');
const logicCounter = computed(() => data.logicCounter ?? 0);
const events = computed<string[]>(() => data.events ?? []);
// View-local state: never leaves the WebView, so only a document reload
// (a fresh instance) clears it.
const viewCounter = ref(0);
const popupOpen = ref(false);
interface UiStackEntry { index: number; name: string; current: boolean }
const reversedPageStack = computed<UiStackEntry[]>(() => [...pageStack.value].reverse());
const modalResult = computed(() => data.modalResult ?? null);
const toastIcon = computed(() => data.toastIcon ?? 'success');
const toastIconLabel = computed(() => data.toastIconLabel ?? 'Success');
const toastIconOptions = computed(() => data.toastIconOptions ?? []);
const toastPosition = computed(() => data.toastPosition ?? 'center');
const toastPositionLabel = computed(() => data.toastPositionLabel ?? 'Center');
const toastPositionOptions = computed(() => data.toastPositionOptions ?? []);
const surfaceMessage = computed(() => data.surfaceDemo?.message ?? '');
const surfaceActive = computed(() => data.surfaceDemo?.active === true);
const surfaceVisible = computed(() => data.surfaceDemo?.visible === true);
const chromeError = computed(() => data.chromeError ?? '');
const appearance = computed(() => data.appearance ?? { preference: 'auto', resolved: 'light' });
const APPEARANCE_OPTIONS = ['auto', 'light', 'dark'] as const;

const toastIconDisplay = computed(() => {
  const match = toastIconOptions.value.find((o: any) => o.value === toastIcon.value);
  return match?.label || toastIconLabel.value || toastIcon.value || 'Select icon';
});

const toastPositionDisplay = computed(() => {
  const match = toastPositionOptions.value.find((o: any) => o.value === toastPosition.value);
  return match?.label || toastPositionLabel.value || toastPosition.value || 'Select position';
});

// Local state
const toastTitle = ref('Hello Toast!');
const toastDuration = ref(2000);
const toastMask = ref(false);
const modalTitle = ref('Alert');
const modalContent = ref('This is a modal dialog');
const modalShowCancel = ref(true);
const modalCancelText = ref('Cancel');
const modalConfirmText = ref('OK');
const navbarTitle = ref('');
const navbarBgColor = ref('');
const navbarTextColor = ref('');
const badgeText = ref('99');
const itemText = ref('New Tab');
const tabColor = ref('#666666');
const tabSelectedColor = ref('#007AFF');
const surfaceKind = ref<'aside' | 'float' | 'window'>('aside');
const surfaceKinds = [
  { id: 'aside', label: 'Aside', hint: 'Docks beside the main and splits it; a compact window folds it into a switchable tab.' },
  { id: 'float', label: 'Float', hint: 'A popup above the main; it does not take layout space (like a dialog).' },
  { id: 'window', label: 'Window', hint: 'A bare standalone window — no sidebar, no shell. Desktop only.' },
] as const;
const surfaceEdge = ref<'left' | 'right' | 'top' | 'bottom'>('right');
const surfaceEdges = ['left', 'right', 'top', 'bottom'] as const;
const surfaceFloatPosition = ref<'center' | 'top' | 'bottom' | 'left' | 'right'>('center');
const surfaceFloatPositions = ['center', 'top', 'bottom', 'left', 'right'] as const;
const surfaceWidth = ref('');
const surfaceHeight = ref('');
// Shown when an entered width/height can't be parsed (so a typo like a
// full-width "％" surfaces instead of silently opening at the wrong size).
const sizeError = ref('');

// Parse a surface size input. Blank means "let the Host pick the default size";
// a bare number is absolute px ("320"); a `%` suffix is a percentage of the
// container ("80%"). Non-blank but unparseable input — a stray letter, a
// full-width "％", a non-positive or out-of-range value — throws so the demo
// reports the mistake instead of silently dropping the dimension (which would
// e.g. quietly turn a 100%/100% float into a centered, default-size one).
function parseSurfaceSize(raw: string, label: string): number | string | undefined {
  const value = raw.trim();
  if (!value) return undefined;
  if (value.endsWith('%')) {
    const pct = Number(value.slice(0, -1).trim());
    if (!Number.isFinite(pct) || pct <= 0 || pct > 100) {
      throw new Error(`${label} must be a percentage between 1% and 100% (got "${value}")`);
    }
    return `${pct}%`;
  }
  const px = Number(value);
  if (!Number.isFinite(px) || px <= 0) {
    throw new Error(`${label} must be a positive px value or a percentage like "80%" (got "${value}")`);
  }
  return px;
}

function handleOpenSurface() {
  let width: number | string | undefined;
  let height: number | string | undefined;
  try {
    width = parseSurfaceSize(surfaceWidth.value, 'Width');
    height = parseSurfaceSize(surfaceHeight.value, 'Height');
  } catch (error) {
    sizeError.value = error instanceof Error ? error.message : String(error);
    return;
  }
  sizeError.value = '';
  openSurfaceDemo({
    verb: surfaceKind.value,
    edge: surfaceEdge.value,
    position: surfaceFloatPosition.value,
    width,
    height,
  });
}

function applyTheme(theme: { color: string; selectedColor: string }) {
  tabColor.value = theme.color;
  tabSelectedColor.value = theme.selectedColor;
  updateTabBarForegrounds(theme);
}
</script>
