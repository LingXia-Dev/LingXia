<template>
  <div class="min-h-screen bg-gray-100">
    <div class="px-4 py-5 space-y-4">
      <header class="bg-gradient-to-r from-blue-500 to-cyan-600 rounded-xl px-4 py-4">
        <div class="text-lg text-white font-bold">{{ pageTitle }}</div>
        <div class="text-xs text-white/85 mt-1">{{ pageSubtitle }}</div>
      </header>

      <template v-if="demoType === 'textarea'">
        <section class="bg-white rounded-xl p-4 space-y-2">
          <div class="text-sm font-semibold text-gray-900">Basic Textarea</div>
          <div class="text-xs text-gray-500">Standard multi-line input</div>
          <div class="rounded-lg border border-gray-300 bg-white overflow-hidden px-2">
            <LxTextarea id="textarea-basic" style="height: 64px;" placeholder="Type here" adjust-position="false" bind-input="onInputChange" />
          </div>
        </section>

        <section class="bg-white rounded-xl p-4 space-y-2">
          <div class="text-sm font-semibold text-gray-900">Auto-height Textarea</div>
          <div class="text-xs text-gray-500">Height grows with content</div>
          <div class="rounded-lg border border-gray-300 bg-white overflow-hidden px-2">
            <LxTextarea id="textarea-autoheight" style="height: 96px;" auto-height placeholder="Type here" bind-input="onInputChange" />
          </div>
        </section>

        <section class="bg-white rounded-xl p-4 space-y-2">
          <div class="text-sm font-semibold text-gray-900">Max Length</div>
          <div class="text-xs text-gray-500">Input length limited to 200</div>
          <div class="rounded-lg border border-gray-300 bg-white overflow-hidden px-2">
            <LxTextarea
              id="textarea-maxlength"
              style="height: 64px;"
              :value="textareaMaxLengthValue"
              :maxlength="200"
              adjust-position="true"
              placeholder="Max length is 200"
              bind-input="onTextareaMaxLengthInput"
            />
          </div>
          <div class="text-xs text-gray-400 text-right">{{ textareaMaxLengthValue.length }}/200</div>
        </section>
      </template>

      <template v-else-if="demoType === 'input'">
        <section class="bg-white rounded-xl p-4 space-y-2">
          <div class="text-sm font-semibold text-gray-900">Basic Input</div>
          <div class="text-xs text-gray-500">Standard single line text input</div>
          <div class="rounded-lg border border-gray-300 bg-white overflow-hidden px-2">
            <LxInput id="input-basic" style="height: 36px;" placeholder="Type here" bind-input="onInputChange" />
          </div>
        </section>

        <section class="bg-white rounded-xl p-4 space-y-2">
          <div class="text-sm font-semibold text-gray-900">Max Length</div>
          <div class="text-xs text-gray-500">Input length limited to 10</div>
          <div class="rounded-lg border border-gray-300 bg-white overflow-hidden px-2">
            <LxInput
              id="input-maxlength"
              style="height: 36px;"
              :value="maxLengthValue"
              :maxlength="10"
              placeholder="Max length is 10"
              bind-input="onMaxLengthInput"
            />
          </div>
          <div class="text-xs text-gray-400 text-right">{{ maxLengthValue.length }}/10</div>
        </section>

        <section class="bg-white rounded-xl p-4 space-y-2">
          <div class="text-sm font-semibold text-gray-900">Realtime Sync</div>
          <div class="text-xs text-gray-500">Input value is synced back to the view each change</div>
          <div class="rounded-lg border border-gray-300 bg-white overflow-hidden px-2">
            <LxInput
              id="input-sync"
              style="height: 36px;"
              :value="syncValue"
              placeholder="Synced to view"
              bind-input="onSyncInput"
            />
          </div>
          <div class="text-xs text-gray-500">Current value: {{ syncValue }}</div>
        </section>

        <section class="bg-white rounded-xl p-4 space-y-2">
          <div class="text-sm font-semibold text-gray-900">Controlled Input</div>
          <div class="text-xs text-gray-500">Two consecutive 1 become a single 2</div>
          <div class="rounded-lg border border-gray-300 bg-white overflow-hidden px-2">
            <LxInput
              id="input-controlled"
              style="height: 36px;"
              :value="controlledValue"
              placeholder="Try typing 1111"
              bind-input="onControlledInput"
            />
          </div>
        </section>

        <section class="bg-white rounded-xl p-4 space-y-2">
          <div class="text-sm font-semibold text-gray-900">Rule Auto Blur</div>
          <div class="text-xs text-gray-500">Type 123 to auto hide keyboard</div>
          <div class="rounded-lg border border-gray-300 bg-white overflow-hidden px-2">
            <LxInput
              id="input-autoblur"
              style="height: 36px;"
              :value="autoBlurValue"
              :focus="autoBlurFocus"
              type="number"
              :maxlength="3"
              placeholder="Type 123"
              bind-input="onAutoBlurInput"
              bind-focus="onAutoBlurFocus"
              bind-blur="onAutoBlurBlur"
            />
          </div>
        </section>

        <section class="bg-white rounded-xl p-4 space-y-2">
          <div class="text-sm font-semibold text-gray-900">Keyboard Control</div>
          <div class="text-xs text-gray-500">Press enter to trigger confirm</div>
          <div class="rounded-lg border border-gray-300 bg-white overflow-hidden px-2">
            <LxInput id="input-confirm" style="height: 36px;" placeholder="Press enter" confirm-type="done" bind-input="onInputChange" />
          </div>
        </section>

        <section class="bg-white rounded-xl p-4 space-y-2">
          <div class="text-sm font-semibold text-gray-900">Number Input</div>
          <div class="text-xs text-gray-500">Only numeric characters should be accepted</div>
          <div class="rounded-lg border border-gray-300 bg-white overflow-hidden px-2">
            <LxInput id="input-number" style="height: 36px;" type="number" placeholder="Numbers only" bind-input="onInputChange" />
          </div>
        </section>

        <section class="bg-white rounded-xl p-4 space-y-2">
          <div class="text-sm font-semibold text-gray-900">Password Input</div>
          <div class="text-xs text-gray-500">Password should stay masked while editing</div>
          <div class="rounded-lg border border-gray-300 bg-white overflow-hidden px-2">
            <LxInput id="input-password" style="height: 36px;" type="password" placeholder="Password" bind-input="onInputChange" />
          </div>
        </section>

        <section class="bg-white rounded-xl p-4 space-y-2">
          <div class="text-sm font-semibold text-gray-900">Digit Input</div>
          <div class="text-xs text-gray-500">Allows decimal point</div>
          <div class="rounded-lg border border-gray-300 bg-white overflow-hidden px-2">
            <LxInput id="input-digit" style="height: 36px;" type="digit" placeholder="Decimal number" bind-input="onInputChange" />
          </div>
        </section>

        <section class="bg-white rounded-xl p-4 space-y-2">
          <div class="text-sm font-semibold text-gray-900">Placeholder Color</div>
          <div class="text-xs text-gray-500">Placeholder text color can be customized</div>
          <div class="rounded-lg border border-gray-300 bg-white overflow-hidden px-2">
            <LxInput
              id="input-placeholder-color"
              style="height: 36px;"
              placeholder-style="color:#ef4444;"
              placeholder="Placeholder should be red"
              bind-input="onInputChange"
            />
          </div>
        </section>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useLingXia } from '@lingxia/core/vue';
import { LxInput, LxTextarea } from '@lingxia/components/vue';
import '../../tailwind.css';

type DemoType = 'input' | 'textarea';

type InputPageData = {
  demoType?: DemoType;
  maxLengthValue?: string;
  textareaMaxLengthValue?: string;
  syncValue?: string;
  controlledValue?: string;
  autoBlurValue?: string;
  autoBlurFocus?: boolean;
};

const { data } = useLingXia() as { data?: InputPageData };

const demoType = computed<DemoType>(() => (data?.demoType === 'textarea' ? 'textarea' : 'input'));
const pageTitle = computed(() => (demoType.value === 'textarea' ? 'Textarea' : 'Input'));
const pageSubtitle = computed(() => (demoType.value === 'textarea' ? 'Native textarea behavior' : 'Native input behavior'));
const maxLengthValue = computed(() => data?.maxLengthValue || '');
const textareaMaxLengthValue = computed(() => data?.textareaMaxLengthValue || '');
const syncValue = computed(() => data?.syncValue || '');
const controlledValue = computed(() => data?.controlledValue || '');
const autoBlurValue = computed(() => data?.autoBlurValue || '');
const autoBlurFocus = computed(() => data?.autoBlurFocus || false);
</script>
