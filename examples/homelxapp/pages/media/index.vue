<template>
  <div class="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100">
    <div class="px-4 py-6 space-y-5">
      <!-- Page Header -->
      <div class="w-full bg-white rounded-2xl shadow-sm border border-gray-100 p-6 text-center">
        <div class="space-y-3">
          <h1 class="text-xl font-semibold text-gray-800">{{ pageInfo.title }}</h1>
          <div v-if="pageInfo.subtitle" class="flex items-center justify-center gap-2">
            <div class="h-px w-8 bg-gradient-to-r from-transparent via-blue-400 to-transparent"></div>
            <p class="text-sm font-medium text-blue-600">{{ pageInfo.subtitle }}</p>
            <div class="h-px w-8 bg-gradient-to-r from-transparent via-blue-400 to-transparent"></div>
          </div>
          <p v-if="pageInfo.description" class="text-sm text-gray-500 max-w-md mx-auto leading-relaxed">
            {{ pageInfo.description }}
          </p>
        </div>
      </div>

      <!-- Scan Code Mode -->
      <template v-if="isScanMode">
        <!-- Settings -->
        <div class="w-full bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="divide-y divide-gray-100">
            <button
              type="button"
              class="group flex w-full items-center gap-4 px-6 py-4 text-sm transition-all hover:bg-gradient-to-r hover:from-blue-50/50 hover:to-transparent active:scale-[0.99]"
              @click="openScanSourcePicker"
            >
              <span class="text-gray-600 font-medium flex-shrink-0">Source</span>
              <div class="flex-1 border-b border-dashed border-gray-200"></div>
              <span class="font-semibold text-gray-800 transition-colors group-hover:text-blue-600">
                {{ scanOnlyCamera ? 'Camera' : 'Camera & Album' }}
              </span>
              <span class="text-gray-400 text-lg transition-transform group-hover:translate-x-0.5">›</span>
            </button>
            <button
              type="button"
              class="group flex w-full items-center gap-4 px-6 py-4 text-sm transition-all hover:bg-gradient-to-r hover:from-blue-50/50 hover:to-transparent active:scale-[0.99]"
              @click="openScanTypePicker"
            >
              <span class="text-gray-600 font-medium flex-shrink-0">Scan Type</span>
              <div class="flex-1 border-b border-dashed border-gray-200"></div>
              <span class="font-semibold text-gray-800 transition-colors group-hover:text-blue-600">
                {{ scanTypeKey }}
              </span>
              <span class="text-gray-400 text-lg transition-transform group-hover:translate-x-0.5">›</span>
            </button>
          </div>
        </div>

        <!-- Scan Result -->
        <div class="w-full bg-white rounded-2xl shadow-sm border border-gray-100 p-6">
          <div class="space-y-4">
            <div class="space-y-2">
              <h3 class="text-sm font-semibold text-gray-700 flex items-center gap-2">
                <span class="w-1 h-4 bg-blue-500 rounded-full"></span>
                Scan Result
              </h3>
              <div class="min-h-[8rem] w-full rounded-xl bg-gradient-to-br from-gray-50 to-gray-100 px-5 py-4 text-base text-gray-900 break-words border border-gray-200 font-mono">
                <span v-if="scanResult">{{ scanResult }}</span>
                <span v-else class="text-gray-400 italic">No result yet</span>
              </div>
              <div class="text-xs text-gray-500 flex items-center gap-2">
                <span class="font-medium">Type:</span>
                <span class="px-2 py-1 bg-gray-100 rounded-md">{{ scanType || '--' }}</span>
              </div>
            </div>

            <button
              @click="startScan"
              :disabled="scanBusy"
              :class="[
                'w-full px-5 py-3 text-sm font-medium rounded-xl shadow-sm transition-all duration-200 active:scale-[0.98]',
                scanBusy ? 'bg-gray-400 text-white cursor-not-allowed' : 'bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white'
              ]"
            >
              {{ scanBusy ? 'Scanning...' : 'Start Scan' }}
            </button>
          </div>
        </div>
      </template>

      <!-- Image/Video Mode -->
      <template v-else>
        <!-- Settings Row (for image/video) -->
        <div v-if="settingRows.length > 0" class="w-full bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
          <div class="divide-y divide-gray-100">
            <button
              v-for="row in settingRows"
              :key="row.label"
              type="button"
              class="group flex w-full items-center gap-4 px-6 py-4 text-sm transition-all hover:bg-gradient-to-r hover:from-blue-50/50 hover:to-transparent active:scale-[0.99]"
              @click="row.action"
            >
              <span class="text-gray-600 font-medium flex-shrink-0">{{ row.label }}</span>
              <div class="flex-1 border-b border-dashed border-gray-200"></div>
              <span class="font-semibold text-gray-800 transition-colors group-hover:text-blue-600">{{ row.value }}</span>
              <span class="text-gray-400 text-lg transition-transform group-hover:translate-x-0.5">›</span>
            </button>
          </div>
        </div>

        <!-- Main Content Card -->
        <div class="w-full bg-white rounded-2xl shadow-sm border border-gray-100 p-6">
          <!-- Image Info Mode -->
          <template v-if="isImageInfoMode">
            <div class="space-y-5">
              <button
                @click="pickImageForInfo"
                :disabled="imageInfoBusy"
                :class="[
                  'w-full px-5 py-3 text-sm font-medium rounded-xl shadow-sm transition-all duration-200 active:scale-[0.98]',
                  imageInfoBusy ? 'bg-gray-400 text-white cursor-not-allowed' : 'bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white'
                ]"
              >
                {{ imageInfoBusy ? 'Getting Info…' : 'Pick Image' }}
              </button>

              <div v-if="imageInfoError" class="flex items-center gap-2 text-sm text-red-600 bg-red-50 px-4 py-3 rounded-xl">
                <span>⚠️</span>
                <span>{{ imageInfoError }}</span>
              </div>

              <div v-if="imageInfoResult" class="space-y-4">
                <div class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-5 space-y-4">
                  <h3 class="text-sm font-semibold text-gray-700 flex items-center gap-2">
                    <span class="w-1 h-4 bg-blue-500 rounded-full"></span>
                    Source Image
                  </h3>
                  <div class="space-y-3">
                    <div class="flex items-center justify-between text-sm">
                      <span class="text-gray-600">Dimensions</span>
                      <span class="font-semibold text-gray-800">{{ imageInfoResult.width ?? '--' }} × {{ imageInfoResult.height ?? '--' }}</span>
                    </div>
                    <div class="flex items-center justify-between text-sm">
                      <span class="text-gray-600">Type</span>
                      <span class="font-semibold text-gray-800">{{ imageInfoResult.type || '--' }}</span>
                    </div>
                    <div class="flex items-center justify-between text-sm">
                      <span class="text-gray-600">File Size</span>
                      <span class="font-semibold text-gray-800">{{ formatFileSize(imageInfoResult.size || 0) }}</span>
                    </div>
                  </div>
                  <div v-if="imageInfoResult.path" class="pt-4 border-t border-gray-200 space-y-1">
                    <div class="text-xs font-medium text-gray-700">Path</div>
                    <div class="text-[11px] text-gray-500 break-all bg-gray-100 px-3 py-2 rounded-lg">
                      {{ imageInfoResult.path }}
                    </div>
                  </div>
                </div>

                <div class="grid grid-cols-3 gap-3">
                  <label class="flex flex-col gap-1">
                    <span class="text-xs font-medium text-gray-600">Quality</span>
                    <input
                      type="number"
                      min="0"
                      max="100"
                      :value="compressQuality"
                      class="w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-800 focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-200"
                      @input="onCompressQualityInput"
                    />
                  </label>
                  <label class="flex flex-col gap-1">
                    <span class="text-xs font-medium text-gray-600">Width</span>
                    <input
                      type="number"
                      min="0"
                      :value="compressedWidth"
                      :placeholder="String(imageInfoResult.width || '')"
                      class="w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-800 focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-200"
                      @input="onCompressedWidthInput"
                    />
                  </label>
                  <label class="flex flex-col gap-1">
                    <span class="text-xs font-medium text-gray-600">Height</span>
                    <input
                      type="number"
                      min="0"
                      :value="compressedHeight"
                      :placeholder="String(imageInfoResult.height || '')"
                      class="w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-800 focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-200"
                      @input="onCompressedHeightInput"
                    />
                  </label>
                </div>

                <button
                  @click="compressSelectedImage"
                  :disabled="compressing"
                  :class="[
                    'w-full px-5 py-3 text-sm font-medium rounded-xl shadow-sm transition-all duration-200 active:scale-[0.98]',
                    compressing ? 'bg-gray-400 text-white cursor-not-allowed' : 'bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white'
                  ]"
                >
                  {{ compressing ? 'Compressing…' : 'Compress Image' }}
                </button>

                <div v-if="compressError" class="flex items-center gap-2 text-sm text-red-600 bg-red-50 px-4 py-3 rounded-xl">
                  <span>⚠️</span>
                  <span>{{ compressError }}</span>
                </div>

                <div v-if="compressResult" class="space-y-4">
                  <div class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-5 space-y-4">
                    <h3 class="text-sm font-semibold text-gray-700 flex items-center gap-2">
                      <span class="w-1 h-4 bg-blue-500 rounded-full"></span>
                      Compressed Image
                    </h3>
                    <div class="space-y-3">
                      <div class="flex items-center justify-between text-sm">
                        <span class="text-gray-600">Dimensions</span>
                        <span class="font-semibold text-gray-800">{{ compressResult.width ?? '--' }} × {{ compressResult.height ?? '--' }}</span>
                      </div>
                      <div class="flex items-center justify-between text-sm">
                        <span class="text-gray-600">Type</span>
                        <span class="font-semibold text-gray-800">{{ compressResult.type || '--' }}</span>
                      </div>
                      <div class="flex items-center justify-between text-sm">
                        <span class="text-gray-600">File Size</span>
                        <span class="font-semibold text-gray-800">{{ formatFileSize(compressResult.size || 0) }}</span>
                      </div>
                    </div>
                    <div v-if="compressResult.path" class="pt-4 border-t border-gray-200 space-y-1">
                      <div class="text-xs font-medium text-gray-700">Path</div>
                      <div class="text-[11px] text-gray-500 break-all bg-gray-100 px-3 py-2 rounded-lg">
                        {{ compressResult.path }}
                      </div>
                    </div>
                  </div>

                  <button
                    @click="previewCompressedImage"
                    class="w-full px-5 py-3 text-sm font-medium rounded-xl shadow-sm transition-all duration-200 active:scale-[0.98] bg-gradient-to-r from-gray-600 to-gray-500 hover:from-gray-500 hover:to-gray-600 text-white"
                  >
                    Preview Image
                  </button>
                </div>
              </div>
            </div>
          </template>

          <!-- Video Tools Mode -->
          <template v-else-if="isVideoToolsMode">
            <div class="space-y-5">
              <button
                @click="pickVideoForTools"
                :disabled="thumbnailBusy || videoInfoBusy"
                :class="[
                  'w-full px-5 py-3 text-sm font-medium rounded-xl shadow-sm transition-all duration-200 active:scale-[0.98]',
                  (thumbnailBusy || videoInfoBusy) ? 'bg-gray-400 text-white cursor-not-allowed' : 'bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white'
                ]"
              >
                {{ (thumbnailBusy || videoInfoBusy) ? 'Loading…' : (thumbnailVideoPath ? 'Pick Another Video' : 'Pick Video') }}
              </button>

              <div v-if="thumbnailVideoPath" class="rounded-xl border border-gray-200 bg-gray-50 px-4 py-3 text-xs text-gray-600 break-all">
                {{ thumbnailVideoPath }}
              </div>

              <div v-if="videoInfoError" class="flex items-center gap-2 text-sm text-red-600 bg-red-50 px-4 py-3 rounded-xl">
                <span>⚠️</span>
                <span>{{ videoInfoError }}</span>
              </div>

              <div v-if="videoInfoResult" class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-5 space-y-4">
                <h3 class="text-sm font-semibold text-gray-700 flex items-center gap-2">
                  <span class="w-1 h-4 bg-blue-500 rounded-full"></span>
                  Video Information
                </h3>
                <div class="space-y-3">
                  <div class="flex items-center justify-between text-sm">
                    <span class="text-gray-600">Resolution</span>
                    <span class="font-semibold text-gray-800">{{ videoInfoResult.width ?? '--' }} × {{ videoInfoResult.height ?? '--' }}</span>
                  </div>
                  <div class="flex items-center justify-between text-sm">
                    <span class="text-gray-600">Duration</span>
                    <span class="font-semibold text-gray-800">{{ formatDuration(videoInfoResult.durationMs) }}</span>
                  </div>
                  <div class="flex items-center justify-between text-sm">
                    <span class="text-gray-600">Rotation</span>
                    <span class="font-semibold text-gray-800">{{ videoInfoResult.rotation ?? '--' }}</span>
                  </div>
                  <div class="flex items-center justify-between text-sm">
                    <span class="text-gray-600">Bitrate</span>
                    <span class="font-semibold text-gray-800">{{ formatBitrate(videoInfoResult.bitrate) }}</span>
                  </div>
                  <div class="flex items-center justify-between text-sm">
                    <span class="text-gray-600">FPS</span>
                    <span class="font-semibold text-gray-800">{{ videoInfoResult.fps ?? '--' }}</span>
                  </div>
                  <div class="flex items-center justify-between text-sm">
                    <span class="text-gray-600">Type</span>
                    <span class="font-semibold text-gray-800">{{ videoInfoResult.type || '--' }}</span>
                  </div>
                  <div class="flex items-center justify-between text-sm">
                    <span class="text-gray-600">Size</span>
                    <span class="font-semibold text-gray-800">{{ formatFileSize(videoInfoResult.size || 0) }}</span>
                  </div>
                </div>
                <div v-if="videoInfoResult.path" class="pt-4 border-t border-gray-200 space-y-1">
                  <div class="text-xs font-medium text-gray-700">Path</div>
                  <div class="text-[11px] text-gray-500 break-all bg-gray-100 px-3 py-2 rounded-lg">
                    {{ videoInfoResult.path }}
                  </div>
                </div>
              </div>

              <div v-if="thumbnailSourceInfo" class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-5 space-y-4">
                <h3 class="text-sm font-semibold text-gray-700 flex items-center gap-2">
                  <span class="w-1 h-4 bg-blue-500 rounded-full"></span>
                  Source Video
                </h3>
                <div class="space-y-3">
                  <div class="flex items-center justify-between text-sm">
                    <span class="text-gray-600">Resolution</span>
                    <span class="font-semibold text-gray-800">{{ thumbnailSourceInfo.width ?? '--' }} × {{ thumbnailSourceInfo.height ?? '--' }}</span>
                  </div>
                  <div class="flex items-center justify-between text-sm">
                    <span class="text-gray-600">Duration</span>
                    <span class="font-semibold text-gray-800">{{ formatDuration(thumbnailSourceInfo.durationMs) }}</span>
                  </div>
                  <div class="flex items-center justify-between text-sm">
                    <span class="text-gray-600">Type</span>
                    <span class="font-semibold text-gray-800">{{ thumbnailSourceInfo.type || '--' }}</span>
                  </div>
                </div>
              </div>

              <div class="text-xs text-gray-600 bg-blue-50 border border-blue-100 rounded-xl px-3 py-2 space-y-1">
                <div>Quality: 0-100</div>
                <div>Time (ms): 0 means first frame</div>
                <div>Max Width / Max Height unit: px</div>
                <div>Leave Max Width/Height empty to keep original size</div>
                <div>Thumbnail is scaled proportionally, not cropped</div>
                <div>If Max Width/Height exceeds source, it is clamped automatically</div>
              </div>

              <div class="grid grid-cols-2 gap-3">
                <label class="flex flex-col gap-1">
                  <span class="text-xs font-medium text-gray-600">Quality</span>
                  <input
                    type="number"
                    min="0"
                    max="100"
                    placeholder="80"
                    :value="thumbnailQuality"
                    class="w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-800 focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-200"
                    @input="onThumbnailQualityInput"
                  />
                </label>
                <label class="flex flex-col gap-1">
                  <span class="text-xs font-medium text-gray-600">Time (ms)</span>
                  <input
                    type="text"
                    placeholder="0"
                    :value="thumbnailTimeMs"
                    class="w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-800 focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-200"
                    @input="onThumbnailTimeInput"
                  />
                </label>
                <label class="flex flex-col gap-1">
                  <span class="text-xs font-medium text-gray-600">Max Width (px)</span>
                  <input
                    type="text"
                    :placeholder="thumbnailSourceInfo?.width ? `e.g. ${thumbnailSourceInfo.width}` : 'leave empty'"
                    :value="thumbnailMaxWidth"
                    class="w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-800 focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-200"
                    @input="onThumbnailMaxWidthInput"
                  />
                </label>
                <label class="flex flex-col gap-1">
                  <span class="text-xs font-medium text-gray-600">Max Height (px)</span>
                  <input
                    type="text"
                    :placeholder="thumbnailSourceInfo?.height ? `e.g. ${thumbnailSourceInfo.height}` : 'leave empty'"
                    :value="thumbnailMaxHeight"
                    class="w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-800 focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-200"
                    @input="onThumbnailMaxHeightInput"
                  />
                </label>
              </div>

              <button
                @click="createVideoThumbnail"
                :disabled="thumbnailBusy"
                :class="[
                  'w-full px-5 py-3 text-sm font-medium rounded-xl shadow-sm transition-all duration-200 active:scale-[0.98]',
                  thumbnailBusy ? 'bg-gray-400 text-white cursor-not-allowed' : 'bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white'
                ]"
              >
                {{ thumbnailBusy ? 'Generating…' : 'Generate Thumbnail' }}
              </button>

              <div v-if="thumbnailError" class="flex items-center gap-2 text-sm text-red-600 bg-red-50 px-4 py-3 rounded-xl">
                <span>⚠️</span>
                <span>{{ thumbnailError }}</span>
              </div>

              <div v-if="thumbnailResult?.tempFilePath" class="space-y-4">
                <div class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-5 space-y-4">
                  <h3 class="text-sm font-semibold text-gray-700 flex items-center gap-2">
                    <span class="w-1 h-4 bg-blue-500 rounded-full"></span>
                    Thumbnail Result
                  </h3>
                  <div class="space-y-3">
                    <div class="flex items-center justify-between text-sm">
                      <span class="text-gray-600">Width</span>
                      <span class="font-semibold text-gray-800">{{ thumbnailResult.width ?? '--' }} px</span>
                    </div>
                    <div class="flex items-center justify-between text-sm">
                      <span class="text-gray-600">Height</span>
                      <span class="font-semibold text-gray-800">{{ thumbnailResult.height ?? '--' }} px</span>
                    </div>
                    <div class="flex items-center justify-between text-sm">
                      <span class="text-gray-600">Type</span>
                      <span class="font-semibold text-gray-800">{{ thumbnailResult.type || '--' }}</span>
                    </div>
                  </div>
                  <div class="space-y-2">
                    <img
                      :src="thumbnailResult.tempFilePath"
                      alt="thumbnail"
                      class="w-full rounded-lg border border-gray-200 bg-gray-100"
                    />
                    <div class="text-[11px] text-gray-500 break-all bg-gray-100 px-3 py-2 rounded-lg">
                      {{ thumbnailResult.tempFilePath }}
                    </div>
                  </div>
                </div>

                <button
                  @click="previewVideoThumbnail"
                  class="w-full px-5 py-3 text-sm font-medium rounded-xl shadow-sm transition-all duration-200 active:scale-[0.98] bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white"
                >
                  Preview Thumbnail
                </button>
              </div>

              <div class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-5 space-y-4">
                <h3 class="text-sm font-semibold text-gray-700 flex items-center gap-2">
                  <span class="w-1 h-4 bg-indigo-500 rounded-full"></span>
                  Compress Video
                </h3>

                <div class="text-xs text-gray-600 bg-indigo-50 border border-indigo-100 rounded-lg px-3 py-2 space-y-1">
                  <div>Quality: low / medium / high</div>
                  <div>If quality is set, bitrate/fps/resolution are ignored.</div>
                  <div>Bitrate unit: kbps, FPS unit: frame/s, Resolution range: (0, 1]</div>
                </div>

                <div class="grid grid-cols-2 gap-3">
                  <label class="flex flex-col gap-1">
                    <span class="text-xs font-medium text-gray-600">Quality</span>
                    <input
                      type="text"
                      placeholder="medium"
                      :value="videoCompressQuality"
                      class="w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-800 focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-200"
                      @input="onVideoCompressQualityInput"
                    />
                  </label>
                  <label class="flex flex-col gap-1">
                    <span class="text-xs font-medium text-gray-600">Bitrate (kbps)</span>
                    <input
                      type="number"
                      min="1"
                      placeholder="1200"
                      :value="videoCompressBitrate"
                      class="w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-800 focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-200"
                      @input="onVideoCompressBitrateInput"
                    />
                  </label>
                  <label class="flex flex-col gap-1">
                    <span class="text-xs font-medium text-gray-600">FPS</span>
                    <input
                      type="number"
                      min="1"
                      placeholder="30"
                      :value="videoCompressFps"
                      class="w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-800 focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-200"
                      @input="onVideoCompressFpsInput"
                    />
                  </label>
                  <label class="flex flex-col gap-1">
                    <span class="text-xs font-medium text-gray-600">Resolution Ratio</span>
                    <input
                      type="text"
                      placeholder="0.8"
                      :value="videoCompressResolution"
                      class="w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-800 focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-200"
                      @input="onVideoCompressResolutionInput"
                    />
                  </label>
                </div>

                <button
                  @click="compressSelectedVideo"
                  :disabled="videoCompressBusy"
                  :class="[
                    'w-full px-5 py-3 text-sm font-medium rounded-xl shadow-sm transition-all duration-200 active:scale-[0.98]',
                    videoCompressBusy ? 'bg-gray-400 text-white cursor-not-allowed' : 'bg-gradient-to-r from-green-600 to-green-500 hover:from-green-500 hover:to-green-600 text-white'
                  ]"
                >
                  {{ videoCompressBusy ? 'Compressing…' : 'Compress Video' }}
                </button>

                <div v-if="videoCompressError" class="flex items-center gap-2 text-sm text-red-600 bg-red-50 px-4 py-3 rounded-xl">
                  <span>⚠️</span>
                  <span>{{ videoCompressError }}</span>
                </div>

                <div v-if="videoCompressResult?.tempFilePath" class="space-y-3">
                  <div class="rounded-xl border border-gray-200 bg-gradient-to-br from-gray-50 to-white p-5 space-y-4">
                    <h3 class="text-sm font-semibold text-gray-700 flex items-center gap-2">
                      <span class="w-1 h-4 bg-green-500 rounded-full"></span>
                      Compressed Video
                    </h3>
                    <div class="space-y-3">
                      <div class="flex items-center justify-between text-sm">
                        <span class="text-gray-600">Resolution</span>
                        <span class="font-semibold text-gray-800">{{ videoCompressResult.width ?? '--' }} × {{ videoCompressResult.height ?? '--' }}</span>
                      </div>
                      <div class="flex items-center justify-between text-sm">
                        <span class="text-gray-600">Duration</span>
                        <span class="font-semibold text-gray-800">{{ formatDuration(videoCompressResult.durationMs) }}</span>
                      </div>
                      <div class="flex items-center justify-between text-sm">
                        <span class="text-gray-600">Type</span>
                        <span class="font-semibold text-gray-800">{{ videoCompressResult.type || '--' }}</span>
                      </div>
                      <div class="flex items-center justify-between text-sm">
                        <span class="text-gray-600">File Size</span>
                        <span class="font-semibold text-gray-800">{{ formatFileSize(videoCompressResult.size || 0) }}</span>
                      </div>
                    </div>
                    <div class="text-[11px] text-gray-500 break-all bg-gray-100 px-3 py-2 rounded-lg">
                      {{ videoCompressResult.tempFilePath }}
                    </div>
                  </div>

                  <button
                    @click="previewCompressedVideo"
                    class="w-full px-5 py-3 text-sm font-medium rounded-xl shadow-sm transition-all duration-200 active:scale-[0.98] bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white"
                  >
                    Preview Compressed Video
                  </button>
                </div>
              </div>
            </div>
          </template>

          <!-- Save to Album Mode -->
          <template v-else-if="isSaveToAlbumMode">
            <div class="space-y-5">
              <div class="text-sm text-gray-600 bg-blue-50 px-4 py-3 rounded-xl border border-blue-100">
                📸 Capture photo or video, then save to album. Check your device album to view saved media.
              </div>

              <div class="grid grid-cols-2 gap-4">
                <button
                  @click="captureImageForAlbum"
                  :disabled="saveToAlbumBusy"
                  :class="[
                    'w-full px-5 py-3 text-sm font-medium rounded-xl shadow-sm transition-all duration-200 active:scale-[0.98]',
                    saveToAlbumBusy ? 'bg-gray-400 text-white cursor-not-allowed' : 'bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white'
                  ]"
                >
                  {{ saveToAlbumBusy ? 'Saving...' : 'Capture Image' }}
                </button>
                <button
                  @click="captureVideoForAlbum"
                  :disabled="saveToAlbumBusy"
                  :class="[
                    'w-full px-5 py-3 text-sm font-medium rounded-xl shadow-sm transition-all duration-200 active:scale-[0.98]',
                    saveToAlbumBusy ? 'bg-gray-400 text-white cursor-not-allowed' : 'bg-gradient-to-r from-green-600 to-green-500 hover:from-green-500 hover:to-green-600 text-white'
                  ]"
                >
                  {{ saveToAlbumBusy ? 'Saving...' : 'Capture Video' }}
                </button>
              </div>
            </div>
          </template>

          <!-- Picture/Video Selection Mode -->
          <template v-else>
            <div class="space-y-4">
              <div class="flex items-center justify-between">
                <div class="text-sm text-gray-600">
                  {{ selectedMedia.length ? previewHint : emptyHint }}
                </div>
                <div v-if="countLimit > 0" class="px-3 py-1 bg-blue-50 text-blue-600 text-xs font-semibold rounded-full">
                  {{ counterText }}
                </div>
              </div>

              <!-- Empty State -->
              <div v-if="selectedMedia.length === 0" class="flex flex-col items-center justify-center py-12 text-center">
                <p class="text-sm text-gray-500">{{ emptyHint }}</p>
              </div>

              <!-- Picture Tiles -->
              <div v-else-if="isPictureMode" class="grid grid-cols-3 gap-3">
                <button
                  v-for="(item, index) in selectedMedia"
                  :key="`${item.path}-${index}`"
                  type="button"
                  class="group relative h-32 overflow-hidden rounded-2xl border border-gray-200 bg-gray-50 transition-all hover:shadow-lg hover:scale-[1.02] active:scale-[0.98]"
                  @click="previewSelectedMedia({ item })"
                >
                  <img :src="item.path" alt="" class="h-full w-full object-cover transition-transform group-hover:scale-110" />
                  <div class="absolute inset-0 bg-gradient-to-t from-black/60 via-transparent to-transparent opacity-0 group-hover:opacity-100 transition-opacity"></div>
                  <div class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/80 to-transparent px-3 py-2">
                    <div class="text-[10px] text-white/90 truncate font-medium">Image {{ index + 1 }}</div>
                  </div>
                </button>
                <!-- Add Button -->
                <button
                  v-if="canAddMore"
                  type="button"
                  :disabled="isRunning"
                  :class="[
                    'group flex w-full flex-col items-center justify-center rounded-2xl border-2 border-dashed transition-all h-32',
                    isRunning ? 'cursor-not-allowed opacity-40 border-gray-200 bg-gray-50' : 'border-blue-300 bg-gradient-to-br from-blue-50 to-indigo-50 hover:border-blue-400 hover:from-blue-100 hover:to-indigo-100 active:scale-[0.98]'
                  ]"
                  @click="launchMediaDemo"
                >
                  <span :class="['text-5xl leading-none transition-transform group-hover:scale-110', isRunning ? 'text-gray-400' : 'text-blue-500']">+</span>
                  <span :class="['mt-3 text-xs font-medium uppercase tracking-wider', isRunning ? 'text-gray-400' : 'text-blue-600']">{{ addLabel }}</span>
                </button>
              </div>

              <!-- Video Tiles -->
              <div v-else class="space-y-4">
                <div v-for="(item, index) in selectedMedia" :key="`video-${index}`" class="w-full bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
                  <LxVideo
                    :id="`media-video-${index}`"
                    :src="item.path"
                    controls
                    autoplay
                    muted
                    loop
                    :style="{ width: '100%', height: '224px', display: 'block', backgroundColor: 'black' }"
                  />
                  <div class="px-5 py-4 bg-gradient-to-br from-gray-50 to-white">
                    <div class="flex items-center justify-between gap-4">
                      <div class="flex items-center gap-3 flex-1">
                        <div class="flex items-center justify-center w-10 h-10 rounded-xl bg-gradient-to-br from-blue-50 to-indigo-50">
                          <svg class="w-5 h-5 text-blue-600" fill="currentColor" viewBox="0 0 20 20">
                            <path d="M2 6a2 2 0 012-2h6a2 2 0 012 2v8a2 2 0 01-2 2H4a2 2 0 01-2-2V6zm12.553 1.106A1 1 0 0014 8v4a1 1 0 00.553.894l2 1A1 1 0 0018 13V7a1 1 0 00-1.447-.894l-2 1z" />
                          </svg>
                        </div>
                        <div>
                          <div class="text-sm font-semibold text-gray-800">Video {{ index + 1 }}</div>
                          <div class="text-xs text-gray-500 mt-0.5">Tap to preview fullscreen</div>
                        </div>
                      </div>
                      <button
                        @click="previewSelectedMedia({ item })"
                        class="px-5 py-3 text-sm font-medium bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-600 text-white rounded-xl shadow-sm transition-all duration-200 active:scale-[0.98]"
                      >
                        Preview
                      </button>
                    </div>
                  </div>
                </div>
                <!-- Add Video Button -->
                <button
                  v-if="canAddMore"
                  type="button"
                  :disabled="isRunning"
                  :class="[
                    'group flex w-full flex-col items-center justify-center rounded-2xl border-2 border-dashed transition-all h-48',
                    isRunning ? 'cursor-not-allowed opacity-40 border-gray-200 bg-gray-50' : 'border-blue-300 bg-gradient-to-br from-blue-50 to-indigo-50 hover:border-blue-400 hover:from-blue-100 hover:to-indigo-100 active:scale-[0.98]'
                  ]"
                  @click="launchMediaDemo"
                >
                  <span :class="['text-5xl leading-none transition-transform group-hover:scale-110', isRunning ? 'text-gray-400' : 'text-blue-500']">+</span>
                  <span :class="['mt-3 text-xs font-medium uppercase tracking-wider', isRunning ? 'text-gray-400' : 'text-blue-600']">{{ addLabel }}</span>
                </button>
              </div>

              <!-- Add button when empty -->
              <button
                v-if="selectedMedia.length === 0 && canAddMore"
                type="button"
                :disabled="isRunning"
                :class="[
                  'group flex w-full flex-col items-center justify-center rounded-2xl border-2 border-dashed transition-all',
                  isPictureMode ? 'h-32' : 'h-48',
                  isRunning ? 'cursor-not-allowed opacity-40 border-gray-200 bg-gray-50' : 'border-blue-300 bg-gradient-to-br from-blue-50 to-indigo-50 hover:border-blue-400 hover:from-blue-100 hover:to-indigo-100 active:scale-[0.98]'
                ]"
                @click="launchMediaDemo"
              >
                <span :class="['text-5xl leading-none transition-transform group-hover:scale-110', isRunning ? 'text-gray-400' : 'text-blue-500']">+</span>
                <span :class="['mt-3 text-xs font-medium uppercase tracking-wider', isRunning ? 'text-gray-400' : 'text-blue-600']">{{ addLabel }}</span>
              </button>
            </div>
          </template>
        </div>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useLingXia } from '@lingxia/web-runtime/vue';
import { LxVideo } from 'lingxia-components/vue';
import '../../tailwind.css';

const SOURCE_OPTIONS = [
  { key: 'album', label: 'Album' },
  { key: 'camera', label: 'Camera' },
  { key: 'either', label: 'Album or Camera' },
];

const COUNT_OPTIONS = Array.from({ length: 9 }, (_, i) => ({
  key: String(i + 1),
  label: String(i + 1),
  value: i + 1,
}));

const CAMERA_OPTIONS = [
  { key: 'back', label: 'Rear Camera' },
  { key: 'front', label: 'Front Camera' },
];

const DURATION_OPTIONS = [
  { key: '15', label: '15 seconds', value: 15 },
  { key: '30', label: '30 seconds', value: 30 },
  { key: '60', label: '60 seconds', value: 60 },
];

type MediaItem = { path: string; type: 'image' | 'video' };
type ImageInfoResult = { width?: number; height?: number; type?: string; path?: string; size?: number };
type VideoInfoResult = {
  width?: number;
  height?: number;
  durationMs?: number;
  rotation?: number;
  bitrate?: number;
  fps?: number;
  type?: string;
  path?: string;
  size?: number;
};
type VideoThumbnailSourceInfo = {
  width?: number;
  height?: number;
  durationMs?: number;
  type?: string;
};
type VideoThumbnailResult = {
  tempFilePath?: string;
  width?: number;
  height?: number;
  type?: string;
};
type CompressVideoResult = {
  tempFilePath?: string;
  width?: number;
  height?: number;
  durationMs?: number;
  size?: number;
  type?: string;
};

const {
  data,
  launchMediaDemo,
  previewSelectedMedia,
  openSourcePicker,
  openCountPicker,
  openCameraPicker,
  openDurationPicker,
  openScanSourcePicker,
  openScanTypePicker,
  startScan,
  pickImageForInfo,
  pickVideoForTools,
  onCompressQualityInput,
  onCompressedWidthInput,
  onCompressedHeightInput,
  compressSelectedImage,
  previewCompressedImage,
  onThumbnailQualityInput,
  onThumbnailMaxWidthInput,
  onThumbnailMaxHeightInput,
  onThumbnailTimeInput,
  createVideoThumbnail,
  previewVideoThumbnail,
  onVideoCompressQualityInput,
  onVideoCompressBitrateInput,
  onVideoCompressFpsInput,
  onVideoCompressResolutionInput,
  compressSelectedVideo,
  previewCompressedVideo,
  captureImageForAlbum,
  captureVideoForAlbum,
} = useLingXia();

const mediaTypeInput = computed(() => data?.mediaType || 'image');
const isImageInfoMode = computed(() => mediaTypeInput.value === 'imageInfo');
const isVideoToolsMode = computed(() => mediaTypeInput.value === 'videoTools');
const isSaveToAlbumMode = computed(() => mediaTypeInput.value === 'saveToAlbum');
const mediaType = computed(() => {
  if (mediaTypeInput.value === 'video') return 'video';
  if (mediaTypeInput.value === 'scanCode') return 'scanCode';
  return 'image';
});

const selectedMedia = computed<MediaItem[]>(() => Array.isArray(data?.selectedMedia) ? data.selectedMedia : []);
const isRunning = computed(() => Boolean(data?.isRunning));

const sourceKey = computed(() => data?.sourceKey || SOURCE_OPTIONS[0].key);
const countKey = computed(() => data?.countKey || COUNT_OPTIONS[COUNT_OPTIONS.length - 1].key);
const cameraKey = computed(() => data?.cameraKey || CAMERA_OPTIONS[0].key);
const durationKey = computed(() => data?.durationKey || DURATION_OPTIONS[DURATION_OPTIONS.length - 1].key);

const sourceOption = computed(() => SOURCE_OPTIONS.find(o => o.key === sourceKey.value) || SOURCE_OPTIONS[0]);
const countOption = computed(() => COUNT_OPTIONS.find(o => o.key === countKey.value) || COUNT_OPTIONS[COUNT_OPTIONS.length - 1]);
const cameraOption = computed(() => CAMERA_OPTIONS.find(o => o.key === cameraKey.value) || CAMERA_OPTIONS[0]);
const durationOption = computed(() => DURATION_OPTIONS.find(o => o.key === durationKey.value) || DURATION_OPTIONS[DURATION_OPTIONS.length - 1]);

const countLimit = computed(() => typeof data?.countLimit === 'number' ? data.countLimit : (countOption.value.value ?? 0));
const counterText = computed(() => countLimit.value ? `${selectedMedia.value.length}/${countLimit.value}` : `${selectedMedia.value.length}`);

const isPictureMode = computed(() => mediaType.value === 'image' && !isImageInfoMode.value && !isVideoToolsMode.value);
const isScanMode = computed(() => mediaType.value === 'scanCode');
const isVideoMode = computed(() => mediaType.value === 'video');

const emptyHint = computed(() => data?.emptyHint || (isPictureMode.value ? 'Tap + to pick photos.' : 'Tap + to add a video.'));
const previewHint = computed(() => data?.previewHint || (isPictureMode.value ? 'Tap a photo to preview.' : 'Tap the clip to preview.'));
const headerSubtitle = computed(() => data?.headerSubtitle || 'choose/previewMedia');

const scanResult = computed(() => typeof data?.scanResult === 'string' ? data.scanResult : '');
const scanBusy = computed(() => Boolean(data?.scanBusy));
const scanOnlyCamera = computed(() => Boolean(data?.scanOnlyCamera));
const scanTypeKey = computed(() => data?.scanTypeKey || 'all');
const scanType = computed(() => typeof data?.scanType === 'string' ? data.scanType : '');

const addLabel = computed(() => data?.addLabel || (isPictureMode.value ? 'Add Photo' : 'Add Video'));
const enforceLimit = computed(() => isPictureMode.value ? (countLimit.value || Number.POSITIVE_INFINITY) : 1);
const canAddMore = computed(() => selectedMedia.value.length < enforceLimit.value);

const imageInfoResult = computed<ImageInfoResult | null>(() => data?.imageInfoResult ?? null);
const imageInfoError = computed(() => data?.imageInfoError || '');
const imageInfoBusy = computed(() => Boolean(data?.imageInfoBusy));
const compressQuality = computed(() => {
  const raw = data?.compressQuality ?? '80';
  return typeof raw === 'number' ? String(raw) : raw;
});
const compressedWidth = computed(() => {
  const raw = data?.compressedWidth ?? '';
  return typeof raw === 'number' ? String(raw) : raw;
});
const compressedHeight = computed(() => {
  const raw = data?.compressedHeight ?? '';
  return typeof raw === 'number' ? String(raw) : raw;
});
const compressing = computed(() => Boolean(data?.compressing));
const compressResult = computed<ImageInfoResult | null>(() => data?.compressResult ?? null);
const compressError = computed(() => data?.compressError || '');
const videoInfoResult = computed<VideoInfoResult | null>(() => data?.videoInfoResult ?? null);
const videoInfoError = computed(() => data?.videoInfoError || '');
const videoInfoBusy = computed(() => Boolean(data?.videoInfoBusy));
const thumbnailVideoPath = computed(() => data?.thumbnailVideoPath || '');
const thumbnailSourceInfo = computed<VideoThumbnailSourceInfo | null>(() => data?.thumbnailSourceInfo ?? null);
const thumbnailQuality = computed(() => {
  const raw = data?.thumbnailQuality ?? '80';
  return typeof raw === 'number' ? String(raw) : raw;
});
const thumbnailMaxWidth = computed(() => {
  const raw = data?.thumbnailMaxWidth ?? '';
  return typeof raw === 'number' ? String(raw) : raw;
});
const thumbnailMaxHeight = computed(() => {
  const raw = data?.thumbnailMaxHeight ?? '';
  return typeof raw === 'number' ? String(raw) : raw;
});
const thumbnailTimeMs = computed(() => {
  const raw = data?.thumbnailTimeMs ?? '0';
  return typeof raw === 'number' ? String(raw) : raw;
});
const thumbnailBusy = computed(() => Boolean(data?.thumbnailBusy));
const thumbnailResult = computed<VideoThumbnailResult | null>(() => data?.thumbnailResult ?? null);
const thumbnailError = computed(() => data?.thumbnailError || '');
const videoCompressQuality = computed(() => {
  const raw = data?.videoCompressQuality ?? '';
  return typeof raw === 'number' ? String(raw) : raw;
});
const videoCompressBitrate = computed(() => {
  const raw = data?.videoCompressBitrate ?? '';
  return typeof raw === 'number' ? String(raw) : raw;
});
const videoCompressFps = computed(() => {
  const raw = data?.videoCompressFps ?? '';
  return typeof raw === 'number' ? String(raw) : raw;
});
const videoCompressResolution = computed(() => {
  const raw = data?.videoCompressResolution ?? '';
  return typeof raw === 'number' ? String(raw) : raw;
});
const videoCompressBusy = computed(() => Boolean(data?.videoCompressBusy));
const videoCompressResult = computed<CompressVideoResult | null>(() => data?.videoCompressResult ?? null);
const videoCompressError = computed(() => data?.videoCompressError || '');
const saveToAlbumBusy = computed(() => Boolean(data?.saveToAlbumBusy));

const settingRows = computed(() => {
  if (isScanMode.value || isImageInfoMode.value || isVideoToolsMode.value || isSaveToAlbumMode.value) return [];
  if (isPictureMode.value) {
    return [
      { label: 'Photo Source', value: sourceOption.value.label, action: openSourcePicker },
      { label: 'Count Limit', value: countOption.value.label, action: openCountPicker },
    ];
  }
  return [
    { label: 'Video Source', value: sourceOption.value.label, action: openSourcePicker },
    { label: 'Camera', value: cameraOption.value.label, action: openCameraPicker },
    { label: 'Duration', value: durationOption.value.label, action: openDurationPicker },
  ];
});

const pageInfo = computed(() => {
  if (isScanMode.value) {
    return { title: 'lx.scanCode', subtitle: 'QR & Barcode Scanner', description: 'Scan QR codes and barcodes using camera or album' };
  }
  if (isImageInfoMode.value) {
    return { title: 'lx.getImageInfo / lx.compressImage', subtitle: 'Image Tools', description: 'Get image info and create compressed copy' };
  }
  if (isVideoToolsMode.value) {
    return { title: 'Video Tools', subtitle: 'lx.getVideoInfo / lx.extractVideoThumbnail / lx.compressVideo', description: 'Get video info, generate thumbnail, and create compressed copy' };
  }
  if (isSaveToAlbumMode.value) {
    return { title: 'lx.saveImageToPhotosAlbum / lx.saveVideoToPhotosAlbum', subtitle: 'Save to Album', description: 'Capture photo or video and save to device album' };
  }
  return { title: 'Media Manager', subtitle: headerSubtitle.value, description: undefined };
});

function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
}

function formatDuration(durationMs?: number): string {
  if (!durationMs || durationMs <= 0) return '--';
  return `${(durationMs / 1000).toFixed(2)} s`;
}

function formatBitrate(bitrate?: number): string {
  if (!bitrate || bitrate <= 0) return '--';
  return `${Math.round((bitrate / 1000) * 10) / 10} kbps`;
}
</script>
