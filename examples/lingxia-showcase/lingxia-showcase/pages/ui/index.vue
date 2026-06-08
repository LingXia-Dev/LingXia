<template>
  <div class="h-screen bg-gradient-to-br from-gray-50 to-gray-100 flex flex-col overflow-y-auto">
    <div class="flex-1 overflow-y-auto">
      <div class="pb-6 px-4 pt-6">

        <!-- Navigation Demo -->
        <template v-if="currentType === 'navigation'">
          <div class="mb-4 text-sm text-gray-600 font-semibold">navigateTo/Back, redirectTo</div>
          <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
            <div class="flex items-center justify-between px-5 py-4 hover:bg-gray-50 cursor-pointer border-b border-gray-100" @click="demoNavigateTo">
              <div class="text-sm text-gray-800 font-medium">Navigate to new page</div>
              <span class="text-gray-400 text-lg">›</span>
            </div>
            <div class="flex items-center justify-between px-5 py-4 hover:bg-gray-50 cursor-pointer border-b border-gray-100" @click="demoNavigateBack">
              <div class="text-sm text-gray-800 font-medium">Back to previous page</div>
              <span class="text-gray-400 text-lg">›</span>
            </div>
            <div class="flex items-center justify-between px-5 py-4 hover:bg-gray-50 cursor-pointer border-b border-gray-100" @click="demoRedirectTo">
              <div class="text-sm text-gray-800 font-medium">Open in current page</div>
              <span class="text-gray-400 text-lg">›</span>
            </div>
            <div class="flex items-center justify-between px-5 py-4 hover:bg-gray-50 cursor-pointer" @click="demoSwitchTab">
              <div class="text-sm text-gray-800 font-medium">Jump to Tab page</div>
              <span class="text-gray-400 text-lg">›</span>
            </div>
          </div>

          <!-- Page Stack Info -->
          <div class="mb-5 bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
            <div class="px-5 py-4">
              <div class="flex items-center gap-2 mb-4">
                <span class="w-1 h-5 bg-blue-500 rounded-full"></span>
                <div class="text-sm font-semibold text-gray-700">Current Page Stack</div>
                <span class="ml-auto px-2 py-1 bg-blue-50 text-blue-600 text-xs font-semibold rounded-full">{{ pageStack.length }}</span>
              </div>
              <div class="space-y-2">
                <div v-for="(page, index) in pageStack" :key="index" class="flex flex-col gap-2 py-3 px-4 bg-gradient-to-r from-gray-50 to-white rounded-xl border border-gray-100">
                  <div class="flex items-center gap-3">
                    <span class="flex items-center justify-center w-6 h-6 rounded-full bg-blue-100 text-blue-600 text-xs font-bold">{{ page.index + 1 }}</span>
                    <span class="text-sm text-gray-800 font-medium flex-1 truncate">{{ page.route }}</span>
                  </div>
                  <div v-if="Object.keys(page.options || {}).length > 0" class="ml-9 text-xs text-gray-500 font-mono bg-gray-50 px-3 py-2 rounded-lg break-all">
                    {{ JSON.stringify(page.options, null, 2) }}
                  </div>
                </div>
                <div v-if="pageStack.length === 0" class="text-sm text-gray-500 text-center py-8">No page stack available</div>
              </div>
            </div>
          </div>
        </template>

        <!-- Surface Demo -->
        <template v-if="currentType === 'surface'">
          <div class="mt-4 mb-6 px-4 text-center">
            <h1 class="text-2xl font-light text-gray-800 mb-2">lx.surface.open</h1>
            <div class="w-16 h-0.5 bg-gray-400 mx-auto"></div>
          </div>
          <div class="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
            <div class="px-4 py-4 space-y-4">
              <div class="space-y-3">
                <div class="text-xs text-gray-500 leading-5 bg-gray-50 rounded-lg px-3 py-2">
                  Overlay is cross-platform. Window is shown only on desktop runtimes.
                </div>
                <div>
                  <div class="text-xs uppercase text-gray-500 tracking-wide mb-2">Kind</div>
                  <!-- Segmented control: neutral pill background so the active
                       state doesn't compete visually with the blue CTA below. -->
                  <div :class="['grid gap-1 bg-gray-100 rounded-lg p-1', supportsSurfaceWindow ? 'grid-cols-2' : 'grid-cols-1']">
                    <button v-for="kind in surfaceKinds" :key="kind" type="button"
                      :class="['py-1.5 px-2 text-sm font-medium rounded-md transition-colors', surfaceKind === kind ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500 hover:text-gray-700']"
                      @click="surfaceKind = kind">
                      {{ kind.charAt(0).toUpperCase() + kind.slice(1) }}
                    </button>
                  </div>
                </div>
                <template v-if="surfaceKind === 'overlay'">
                <div>
                  <div class="text-xs uppercase text-gray-500 tracking-wide mb-2">Overlay Size</div>
                  <div class="flex items-center justify-between text-xs text-gray-500 tracking-wide">
                    <span>Width</span>
                    <span class="text-gray-700 font-mono">{{ surfaceWidthRatio.toFixed(2) }}</span>
                  </div>
                  <input type="range" min="0.1" max="1" step="0.05" v-model.number="surfaceWidthRatio" class="w-full mt-2" />
                </div>
                <div>
                  <div class="flex items-center justify-between text-xs text-gray-500 tracking-wide">
                    <span>Height</span>
                    <span class="text-gray-700 font-mono">{{ surfaceHeightRatio.toFixed(2) }}</span>
                  </div>
                  <input type="range" min="0.1" max="1" step="0.05" v-model.number="surfaceHeightRatio" class="w-full mt-2" />
                </div>
                </template>
                <div v-else class="text-xs text-gray-500 leading-5 bg-gray-50 rounded-lg px-3 py-2">
                  Window demo uses a fixed 960 x 720 size. Percent sizes are intentionally not supported for window.
                </div>
                <div class="text-xs text-gray-500 leading-5">
                  {{ surfaceDescription }}
                </div>
                <div v-if="surfaceKind === 'overlay'">
                  <div class="text-xs uppercase text-gray-500 tracking-wide mb-2">
                    Position
                  </div>
                  <div class="grid grid-cols-2 gap-2">
                    <button v-for="pos in surfacePositions" :key="pos" type="button"
                      :class="['py-2 text-sm rounded-lg transition-colors border', surfacePosition === pos ? 'bg-blue-500 border-blue-500 text-white' : 'bg-white border-gray-200 text-gray-600 hover:bg-gray-50']"
                      @click="surfacePosition = pos">
                      {{ pos.charAt(0).toUpperCase() + pos.slice(1) }}
                    </button>
                  </div>
                </div>
              </div>
              <button type="button" :disabled="surfaceActive" @click="openSurfaceDemo({ kind: surfaceKind, widthRatio: surfaceWidthRatio, heightRatio: surfaceHeightRatio, position: surfacePosition, width: 960, height: 720 })"
                class="w-full bg-blue-500 hover:bg-blue-600 disabled:bg-gray-300 disabled:cursor-not-allowed text-white py-2 px-4 rounded-lg text-sm font-medium transition-colors">
                {{ surfaceActive ? `Open ${surfaceKind} (already open)` : `Open ${surfaceKind}` }}
              </button>
              <div v-if="surfaceActive" class="grid grid-cols-3 gap-2">
                <button type="button" :disabled="surfaceVisible" @click="showActiveSurface()"
                  class="bg-emerald-500 hover:bg-emerald-600 disabled:bg-gray-200 disabled:text-gray-500 text-white py-2 px-3 rounded-lg text-sm font-medium transition-colors">
                  Show
                </button>
                <button type="button" :disabled="!surfaceVisible" @click="hideActiveSurface()"
                  class="bg-amber-500 hover:bg-amber-600 disabled:bg-gray-200 disabled:text-gray-500 text-white py-2 px-3 rounded-lg text-sm font-medium transition-colors">
                  Hide
                </button>
                <button type="button" @click="closeActiveSurface()"
                  class="bg-rose-500 hover:bg-rose-600 text-white py-2 px-3 rounded-lg text-sm font-medium transition-colors">
                  Close
                </button>
              </div>
              <div class="text-xs text-gray-500 uppercase tracking-wide">Surface status</div>
              <div class="text-sm text-gray-800 bg-gray-50 rounded-lg px-3 py-2 font-mono break-words">
                {{ surfaceMessage || 'No message received yet.' }}
              </div>
            </div>
          </div>
        </template>

        <!-- Toast Demo -->
        <template v-if="currentType === 'toast'">
          <div class="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">Toast Parameters</div>
          <div class="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
            <div class="px-3 py-3 space-y-3">
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Title</label>
                <input type="text" v-model="toastTitle" class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" placeholder="Enter toast title" />
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Icon</label>
                <button type="button" @click="chooseToastIcon" class="w-full px-3 py-2 border border-gray-300 rounded-md flex items-center justify-between text-left text-gray-800">
                  <span>{{ toastIconDisplay }}</span>
                  <span class="text-xs text-blue-500">Change</span>
                </button>
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Duration (ms)</label>
                <input type="number" v-model.number="toastDuration" class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" min="500" max="10000" step="500" />
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Position</label>
                <button type="button" @click="chooseToastPosition" class="w-full px-3 py-2 border border-gray-300 rounded-md flex items-center justify-between text-left text-gray-800">
                  <span>{{ toastPositionDisplay }}</span>
                  <span class="text-xs text-blue-500">Change</span>
                </button>
              </div>
              <div class="flex items-center">
                <input type="checkbox" id="toastMask" v-model="toastMask" class="h-4 w-4 text-blue-600 border-gray-300 rounded" />
                <label for="toastMask" class="ml-2 block text-sm text-gray-700">Show mask</label>
              </div>
            </div>
          </div>
          <div class="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
            <div class="flex items-center justify-center px-4 py-4 hover:bg-gray-50 cursor-pointer border-b border-gray-100"
              @click="showToastWithParams({ title: toastTitle, icon: toastIcon, duration: toastDuration, position: toastPosition, mask: toastMask })">
              <div class="text-base text-blue-600 font-medium">Show Toast</div>
            </div>
            <div class="flex items-center justify-center px-4 py-4 hover:bg-gray-50 cursor-pointer" @click="hideToast">
              <div class="text-base text-red-600 font-medium">Hide Toast</div>
            </div>
          </div>
        </template>

        <!-- ActionSheet Demo -->
        <template v-if="currentType === 'actionsheet'">
          <div class="mx-1 mt-8 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
            <div class="px-4 py-10 text-base text-blue-600 font-medium text-center cursor-pointer hover:bg-blue-50" @click="showDemoActionSheet">
              Show Action Sheet
            </div>
          </div>
        </template>

        <!-- Modal Demo -->
        <template v-if="currentType === 'modal'">
          <div class="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">Modal Parameters</div>
          <div class="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
            <div class="px-3 py-3 space-y-3">
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Title</label>
                <input type="text" v-model="modalTitle" class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Content</label>
                <textarea v-model="modalContent" class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" rows="3" />
              </div>
              <div class="flex items-center">
                <input type="checkbox" id="modalShowCancel" v-model="modalShowCancel" class="h-4 w-4 text-blue-600 border-gray-300 rounded" />
                <label for="modalShowCancel" class="ml-2 block text-sm text-gray-700">Show cancel button</label>
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Cancel Text</label>
                <input type="text" v-model="modalCancelText" class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Confirm Text</label>
                <input type="text" v-model="modalConfirmText" class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
              </div>
            </div>
          </div>
          <div class="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
            <div class="flex items-center justify-center px-4 py-4 hover:bg-gray-50 cursor-pointer"
              @click="showModalWithParams({ title: modalTitle, content: modalContent, showCancel: modalShowCancel, cancelText: modalCancelText, confirmText: modalConfirmText })">
              <div class="text-base text-blue-600 font-medium">Show Modal</div>
            </div>
          </div>
          <div v-if="modalResult" class="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
            <div class="px-3 py-3">
              <div class="text-sm font-medium text-gray-700 mb-3">Modal Result</div>
              <div class="bg-gray-50 rounded-lg p-3">
                <pre class="text-xs text-gray-600 whitespace-pre-wrap">{{ JSON.stringify(modalResult, null, 2) }}</pre>
              </div>
              <div class="mt-3 text-center text-sm text-red-600 cursor-pointer hover:text-red-800" @click="clearModalResult">Clear Result</div>
            </div>
          </div>
        </template>

        <!-- NavigationBar Demo -->
        <template v-if="currentType === 'navbar'">
          <div class="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">NavigationBar APIs</div>
          <div class="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
            <div class="p-4 space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">Title</label>
                <div class="flex space-x-2">
                  <input type="text" v-model="navbarTitle" placeholder="Enter title" class="flex-1 px-2 py-1.5 text-sm border border-gray-300 rounded focus:outline-none focus:ring-1 focus:ring-blue-500" />
                  <button @click="setNavigationBarTitle({ title: navbarTitle })" class="px-3 py-1.5 text-sm bg-blue-500 text-white rounded hover:bg-blue-600">Set</button>
                </div>
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">Colors</label>
                <div class="space-y-2">
                  <div class="grid grid-cols-2 gap-2">
                    <input type="text" v-model="navbarBgColor" placeholder="Background #ffffff" class="px-2 py-1.5 text-sm border border-gray-300 rounded" />
                    <input type="text" v-model="navbarTextColor" placeholder="Text #000000" class="px-2 py-1.5 text-sm border border-gray-300 rounded" />
                  </div>
                  <button @click="setNavigationBarColor({ backgroundColor: navbarBgColor || '#ffffff', frontColor: navbarTextColor || '#000000' })"
                    class="w-full px-3 py-1.5 text-sm bg-green-500 text-white rounded hover:bg-green-600">Set Colors</button>
                </div>
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">Presets</label>
                <div class="grid grid-cols-2 gap-1.5">
                  <button @click="setNavigationBarTitle({ title: 'Dark Theme' }); setNavigationBarColor({ backgroundColor: '#1f2937', frontColor: '#ffffff' })"
                    class="px-2 py-1.5 bg-gray-800 text-white rounded hover:bg-gray-900 text-xs">Dark</button>
                  <button @click="setNavigationBarTitle({ title: 'Blue Theme' }); setNavigationBarColor({ backgroundColor: '#3b82f6', frontColor: '#ffffff' })"
                    class="px-2 py-1.5 bg-blue-500 text-white rounded hover:bg-blue-600 text-xs">Blue</button>
                  <button @click="setNavigationBarTitle({ title: 'Light Theme' }); setNavigationBarColor({ backgroundColor: '#ffffff', frontColor: '#000000' })"
                    class="px-2 py-1.5 bg-white text-black border border-gray-300 rounded hover:bg-gray-50 text-xs">Light</button>
                  <button @click="setNavigationBarTitle({ title: 'Green Theme' }); setNavigationBarColor({ backgroundColor: '#10b981', frontColor: '#ffffff' })"
                    class="px-2 py-1.5 bg-green-500 text-white rounded hover:bg-green-600 text-xs">Green</button>
                </div>
              </div>
            </div>
          </div>
        </template>

        <!-- TabBar Demo -->
        <template v-if="currentType === 'tabbar'">
          <div class="mt-4 mb-3 px-4 text-sm text-gray-500 font-medium">TabBar APIs</div>

          <!-- Visibility Controls -->
          <div class="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
            <div class="px-4 py-3 border-b border-gray-100">
              <h3 class="text-base font-medium text-gray-900">Visibility Controls</h3>
            </div>
            <div class="p-4 space-y-4">
              <div class="flex space-x-3">
                <button @click="showTabBar()" class="flex-1 bg-green-500 hover:bg-green-600 text-white py-2 px-4 rounded-lg text-sm font-medium">Show TabBar</button>
                <button @click="hideTabBar()" class="flex-1 bg-red-500 hover:bg-red-600 text-white py-2 px-4 rounded-lg text-sm font-medium">Hide TabBar</button>
              </div>
              <div class="pt-2 border-t border-gray-100">
                <label class="block text-sm font-medium text-gray-700 mb-2">Update Tab 1 Text</label>
                <div class="flex space-x-2">
                  <input type="text" v-model="itemText" class="flex-1 px-3 py-2 border border-gray-300 rounded-lg" placeholder="Enter new text" />
                  <button @click="setTabBarItem({ index: 1, text: itemText })" class="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600">Update</button>
                </div>
              </div>
            </div>
          </div>

          <!-- Red Dot Controls -->
          <div class="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
            <div class="px-4 py-3 border-b border-gray-100">
              <h3 class="text-base font-medium text-gray-900">Red Dot Controls</h3>
            </div>
            <div class="p-4">
              <div class="flex space-x-3">
                <button @click="showTabBarRedDot({ index: 1 })" class="flex-1 bg-red-500 hover:bg-red-600 text-white py-2 px-4 rounded-lg text-sm font-medium">Show Red Dot</button>
                <button @click="hideTabBarRedDot({ index: 1 })" class="flex-1 bg-gray-500 hover:bg-gray-600 text-white py-2 px-4 rounded-lg text-sm font-medium">Hide Red Dot</button>
              </div>
            </div>
          </div>

          <!-- Badge Controls -->
          <div class="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
            <div class="px-4 py-3 border-b border-gray-100">
              <h3 class="text-base font-medium text-gray-900">Badge Controls</h3>
            </div>
            <div class="p-4 space-y-3">
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">Badge Text</label>
                <input type="text" v-model="badgeText" class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm" placeholder="Enter badge text" />
              </div>
              <div class="flex space-x-3">
                <button @click="setTabBarBadge({ index: 1, text: badgeText })" class="flex-1 bg-orange-500 hover:bg-orange-600 text-white py-2 px-4 rounded-lg text-sm font-medium">Set Badge</button>
                <button @click="removeTabBarBadge({ index: 1 })" class="flex-1 bg-gray-500 hover:bg-gray-600 text-white py-2 px-4 rounded-lg text-sm font-medium">Remove Badge</button>
              </div>
            </div>
          </div>

          <!-- Style Controls -->
          <div class="mx-1 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
            <div class="px-4 py-3 border-b border-gray-100">
              <h3 class="text-base font-medium text-gray-900">Style Controls</h3>
            </div>
            <div class="p-4 space-y-3">
              <div class="grid grid-cols-2 gap-3">
                <div>
                  <label class="block text-sm font-medium text-gray-700 mb-1">Text Color</label>
                  <div class="flex items-center space-x-2">
                    <div class="w-8 h-8 border border-gray-300 rounded" :style="{ backgroundColor: tabColor }"></div>
                    <input type="text" v-model="tabColor" class="flex-1 px-2 py-1 border border-gray-300 rounded text-sm" />
                  </div>
                </div>
                <div>
                  <label class="block text-sm font-medium text-gray-700 mb-1">Selected Color</label>
                  <div class="flex items-center space-x-2">
                    <div class="w-8 h-8 border border-gray-300 rounded" :style="{ backgroundColor: tabSelectedColor }"></div>
                    <input type="text" v-model="tabSelectedColor" class="flex-1 px-2 py-1 border border-gray-300 rounded text-sm" />
                  </div>
                </div>
                <div>
                  <label class="block text-sm font-medium text-gray-700 mb-1">Background</label>
                  <div class="flex items-center space-x-2">
                    <div class="w-8 h-8 border border-gray-300 rounded" :style="{ backgroundColor: tabBgColor }"></div>
                    <input type="text" v-model="tabBgColor" class="flex-1 px-2 py-1 border border-gray-300 rounded text-sm" />
                  </div>
                </div>
                <div>
                  <label class="block text-sm font-medium text-gray-700 mb-1">Border</label>
                  <div class="flex items-center space-x-2">
                    <div class="w-8 h-8 border border-gray-300 rounded" :style="{ backgroundColor: tabBorderStyle }"></div>
                    <input type="text" v-model="tabBorderStyle" class="flex-1 px-2 py-1 border border-gray-300 rounded text-sm" />
                  </div>
                </div>
              </div>
              <button @click="setTabBarStyle({ color: tabColor, selectedColor: tabSelectedColor, backgroundColor: tabBgColor, borderStyle: tabBorderStyle })"
                class="w-full bg-blue-500 hover:bg-blue-600 text-white py-2 px-4 rounded-lg text-sm font-medium">Apply Custom Style</button>
              <div class="mt-4">
                <label class="block text-sm font-medium text-gray-700 mb-2">Preset Themes</label>
                <div class="grid grid-cols-2 gap-2">
                  <button @click="applyTheme({ color: '#666666', selectedColor: '#007AFF', backgroundColor: '#FFFFFF', borderStyle: '#EEEEEE' })"
                    class="px-3 py-2 bg-gray-100 hover:bg-gray-200 text-gray-700 rounded-lg text-sm font-medium">Default</button>
                  <button @click="applyTheme({ color: '#CCCCCC', selectedColor: '#0A84FF', backgroundColor: '#1C1C1E', borderStyle: '#38383A' })"
                    class="px-3 py-2 bg-gray-800 hover:bg-gray-900 text-white rounded-lg text-sm font-medium">Dark</button>
                  <button @click="applyTheme({ color: '#8E8E93', selectedColor: '#34C759', backgroundColor: '#F2F2F7', borderStyle: '#C6C6C8' })"
                    class="px-3 py-2 bg-green-100 hover:bg-green-200 text-green-700 rounded-lg text-sm font-medium">Green</button>
                  <button @click="applyTheme({ color: '#8E8E93', selectedColor: '#AF52DE', backgroundColor: '#F2F2F7', borderStyle: '#C6C6C8' })"
                    class="px-3 py-2 bg-purple-100 hover:bg-purple-200 text-purple-700 rounded-lg text-sm font-medium">Purple</button>
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
import { ref, computed, watch } from 'vue';
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
  showToastWithParams,
  hideToast,
  showModalWithParams,
  clearModalResult,
  setNavigationBarTitle,
  setNavigationBarColor,
  showTabBarRedDot,
  hideTabBarRedDot,
  setTabBarBadge,
  removeTabBarBadge,
  showTabBar,
  hideTabBar,
  setTabBarStyle,
  setTabBarItem,
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
const supportsSurfaceWindow = computed(() => {
  const bridge = typeof window !== 'undefined' ? (window as any).LingXiaBridge : null;
  return bridge?.platform?.isDesktop?.() === true || data.surfaceDemo?.supportsWindow === true;
});

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
const tabBgColor = ref('#FFFFFF');
const tabBorderStyle = ref('#EEEEEE');
const surfaceWidthRatio = ref(1);
const surfaceHeightRatio = ref(0.6);
const surfacePosition = ref<'center' | 'bottom' | 'left' | 'right' | 'top'>('bottom');
const surfacePositions = ['bottom', 'center', 'left', 'right', 'top'] as const;
const surfaceKind = ref<'overlay' | 'window'>('overlay');
const surfaceKinds = computed(() => supportsSurfaceWindow.value ? ['overlay', 'window'] as const : ['overlay'] as const);
const surfaceDescription = computed(() => surfaceKind.value === 'window'
  ? 'Desktop-only independent window surface. Mobile runtimes reject this kind.'
  : 'Transient overlay surface composited on top of the current app. Use it for lightweight local UI.');

watch(supportsSurfaceWindow, (supports) => {
  if (!supports && surfaceKind.value === 'window') {
    surfaceKind.value = 'overlay';
  }
}, { immediate: true });

function applyTheme(theme: { color: string; selectedColor: string; backgroundColor: string; borderStyle: string }) {
  tabColor.value = theme.color;
  tabSelectedColor.value = theme.selectedColor;
  tabBgColor.value = theme.backgroundColor;
  tabBorderStyle.value = theme.borderStyle;
  setTabBarStyle(theme);
}
</script>
