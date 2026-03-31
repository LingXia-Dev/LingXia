import React from 'react';
import { useLxPage } from '@lingxia/react';
import { LxInput } from '@lingxia/react';
import '../../tailwind.css';

type PageData = {
  maxLengthValue?: string;
  syncValue?: string;
  controlledValue?: string;
  autoBlurValue?: string;
  autoBlurFocus?: boolean;
};

type PageActions = {
  data?: PageData;
  onInputChange?: (detail: { value?: string }) => void;
  onMaxLengthInput?: (detail: { value?: string }) => void;
  onSyncInput?: (detail: { value?: string }) => void;
  onControlledInput?: (detail: { value?: string }) => void;
  onAutoBlurInput?: (detail: { value?: string }) => void;
  onAutoBlurFocus?: (detail: { value?: string }) => void;
  onAutoBlurBlur?: (detail: { value?: string }) => void;
};

function Card(props: { title: string; subtitle?: string; children: React.ReactNode }) {
  return (
    <section className="bg-white rounded-xl p-4 space-y-2">
      <div className="text-sm font-semibold text-gray-900">{props.title}</div>
      {props.subtitle ? <div className="text-xs text-gray-500">{props.subtitle}</div> : null}
      {props.children}
    </section>
  );
}

function FieldBox(props: { children: React.ReactNode }) {
  return (
    <div className="rounded-lg border border-gray-300 bg-white overflow-hidden">
      {props.children}
    </div>
  );
}

export default function InputPage() {
  const { data, actions } = useLxPage();
  const {
    onInputChange,
    onMaxLengthInput,
    onSyncInput,
    onControlledInput,
    onAutoBlurInput,
    onAutoBlurFocus,
    onAutoBlurBlur,
  } = actions;

  return (
    <div className="min-h-screen bg-gray-100">
      <div className="px-4 py-5 space-y-4">
        <header className="bg-gradient-to-r from-blue-500 to-cyan-600 rounded-xl px-4 py-4">
          <div className="text-lg text-white font-bold">Input</div>
          <div className="text-xs text-white/85 mt-1">Cross-platform input component</div>
        </header>

        <Card title="Basic Input" subtitle="Tap inside the box below">
          <FieldBox>
            <LxInput id="input-basic" style={{ height: '36px' }} placeholder="Type here" onInput={onInputChange} />
          </FieldBox>
        </Card>

        <Card title="Max Length" subtitle="Input length limited to 10">
          <FieldBox>
            <LxInput
              id="input-maxlength"
              style={{ height: '36px' }}
              value={data?.maxLengthValue || ''}
              maxlength={10}
              placeholder="Max length is 10"
              onInput={onMaxLengthInput}
            />
          </FieldBox>
          <div className="text-xs text-gray-400 text-right">{(data?.maxLengthValue || '').length}/10</div>
        </Card>

        <Card title="Realtime Sync" subtitle="Input value is synced back to the view each change">
          <FieldBox>
            <LxInput
              id="input-sync"
              style={{ height: '36px' }}
              value={data?.syncValue || ''}
              placeholder="Synced to view"
              onInput={onSyncInput}
            />
          </FieldBox>
          <div className="text-xs text-gray-500">Current value: {data?.syncValue || ''}</div>
        </Card>

        <Card title="Controlled Input" subtitle="Two consecutive 1 become a single 2">
          <FieldBox>
            <LxInput
              id="input-controlled"
              style={{ height: '36px' }}
              value={data?.controlledValue || ''}
              placeholder="Try typing 1111"
              onInput={onControlledInput}
            />
          </FieldBox>
        </Card>

        <Card title="Rule Auto Blur" subtitle="Type 123 to auto hide keyboard">
          <FieldBox>
            <LxInput
              id="input-autoblur"
              style={{ height: '36px' }}
              value={data?.autoBlurValue || ''}
              focus={data?.autoBlurFocus}
              type="number"
              maxlength={3}
              placeholder="Type 123"
              onInput={onAutoBlurInput}
              onFocus={onAutoBlurFocus}
              onBlur={onAutoBlurBlur}
            />
          </FieldBox>
        </Card>

        <Card title="Keyboard Control" subtitle="Press enter to trigger confirm">
          <FieldBox>
            <LxInput id="input-confirm" style={{ height: '36px' }} placeholder="Press enter" confirmType="done" onInput={onInputChange} />
          </FieldBox>
        </Card>

        <Card title="Number Input" subtitle="Only numeric characters should be accepted">
          <FieldBox>
            <LxInput id="input-number" style={{ height: '36px' }} type="number" placeholder="Numbers only" onInput={onInputChange} />
          </FieldBox>
        </Card>

        <Card title="Password Input" subtitle="Password should stay masked while editing">
          <FieldBox>
            <LxInput id="input-password" style={{ height: '36px' }} type="password" placeholder="Password" onInput={onInputChange} />
          </FieldBox>
        </Card>

        <Card title="Digit Input" subtitle="Allows decimal point">
          <FieldBox>
            <LxInput id="input-digit" style={{ height: '36px' }} type="digit" placeholder="Decimal number" onInput={onInputChange} />
          </FieldBox>
        </Card>

        <Card title="Placeholder Color" subtitle="Placeholder text color can be customized">
          <FieldBox>
            <LxInput
              id="input-placeholder-color"
              style={{ height: '36px' }}
              placeholderStyle="color:#ef4444;"
              placeholder="Placeholder should be red"
              onInput={onInputChange}
            />
          </FieldBox>
        </Card>
      </div>
    </div>
  );
}
