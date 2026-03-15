import React from 'react';
import { useLingXia } from '@lingxia/core/react';
import { LxInput, LxTextarea } from '@lingxia/components/react';
import '../../tailwind.css';

type DemoType = 'input' | 'textarea';

type PageData = {
  demoType?: DemoType;
  maxLengthValue?: string;
  textareaMaxLengthValue?: string;
  syncValue?: string;
  controlledValue?: string;
  autoBlurValue?: string;
  autoBlurFocus?: boolean;
};

type PageActions = {
  data?: PageData;
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

function InputDemo(props: { data?: PageData }) {
  return (
    <>
      <Card title="Basic Input" subtitle="Tap inside the box below">
        <FieldBox>
          <LxInput id="input-basic" style={{ height: '36px' }} placeholder="Type here" bindInput="onInputChange" />
        </FieldBox>
      </Card>

      <Card title="Max Length" subtitle="Input length limited to 10">
        <FieldBox>
          <LxInput
            id="input-maxlength"
            style={{ height: '36px' }}
            value={props.data?.maxLengthValue || ''}
            maxlength={10}
            placeholder="Max length is 10"
            bindInput="onMaxLengthInput"
          />
        </FieldBox>
        <div className="text-xs text-gray-400 text-right">{(props.data?.maxLengthValue || '').length}/10</div>
      </Card>

      <Card title="Realtime Sync" subtitle="Input value is synced back to the view each change">
        <FieldBox>
          <LxInput
            id="input-sync"
            style={{ height: '36px' }}
            value={props.data?.syncValue || ''}
            placeholder="Synced to view"
            bindInput="onSyncInput"
          />
        </FieldBox>
        <div className="text-xs text-gray-500">Current value: {props.data?.syncValue || ''}</div>
      </Card>

      <Card title="Controlled Input" subtitle="Two consecutive 1 become a single 2">
        <FieldBox>
          <LxInput
            id="input-controlled"
            style={{ height: '36px' }}
            value={props.data?.controlledValue || ''}
            placeholder="Try typing 1111"
            bindInput="onControlledInput"
          />
        </FieldBox>
      </Card>

      <Card title="Rule Auto Blur" subtitle="Type 123 to auto hide keyboard">
        <FieldBox>
          <LxInput
            id="input-autoblur"
            style={{ height: '36px' }}
            value={props.data?.autoBlurValue || ''}
            focus={props.data?.autoBlurFocus}
            type="number"
            maxlength={3}
            placeholder="Type 123"
            bindInput="onAutoBlurInput"
            bindFocus="onAutoBlurFocus"
            bindBlur="onAutoBlurBlur"
          />
        </FieldBox>
      </Card>

      <Card title="Keyboard Control" subtitle="Press enter to trigger confirm">
        <FieldBox>
          <LxInput id="input-confirm" style={{ height: '36px' }} placeholder="Press enter" confirmType="done" bindInput="onInputChange" />
        </FieldBox>
      </Card>

      <Card title="Number Input" subtitle="Only numeric characters should be accepted">
        <FieldBox>
          <LxInput id="input-number" style={{ height: '36px' }} type="number" placeholder="Numbers only" bindInput="onInputChange" />
        </FieldBox>
      </Card>

      <Card title="Password Input" subtitle="Password should stay masked while editing">
        <FieldBox>
          <LxInput id="input-password" style={{ height: '36px' }} type="password" placeholder="Password" bindInput="onInputChange" />
        </FieldBox>
      </Card>

      <Card title="Digit Input" subtitle="Allows decimal point">
        <FieldBox>
          <LxInput id="input-digit" style={{ height: '36px' }} type="digit" placeholder="Decimal number" bindInput="onInputChange" />
        </FieldBox>
      </Card>

      <Card title="Placeholder Color" subtitle="Placeholder text color can be customized">
        <FieldBox>
          <LxInput
            id="input-placeholder-color"
            style={{ height: '36px' }}
            placeholderStyle="color:#ef4444;"
            placeholder="Placeholder should be red"
            bindInput="onInputChange"
          />
        </FieldBox>
      </Card>
    </>
  );
}

function TextareaDemo(props: { data?: PageData }) {
  return (
    <>
      <Card title="Basic Textarea" subtitle="Standard multi-line input">
        <FieldBox>
          <LxTextarea
            id="textarea-basic"
            style={{ height: '64px' }}
            placeholder="Type here"
            adjustPosition={false}
            bindInput="onInputChange"
          />
        </FieldBox>
      </Card>

      <Card title="Auto-height Textarea" subtitle="Height grows with content">
        <FieldBox>
          <LxTextarea id="textarea-autoheight" style={{ height: '96px' }} autoHeight placeholder="Type here" bindInput="onInputChange" />
        </FieldBox>
      </Card>

      <Card title="Max Length" subtitle="Input length limited to 200">
        <FieldBox>
          <LxTextarea
            id="textarea-maxlength"
            style={{ height: '64px' }}
            value={props.data?.textareaMaxLengthValue || ''}
            maxlength={200}
            placeholder="Max length is 200"
            adjustPosition
            bindInput="onTextareaMaxLengthInput"
          />
        </FieldBox>
        <div className="text-xs text-gray-400 text-right">{(props.data?.textareaMaxLengthValue || '').length}/200</div>
      </Card>
    </>
  );
}

export default function InputPage() {
  const { data } = useLingXia() as PageActions;
  const demoType: DemoType | null =
    data?.demoType === 'textarea' ? 'textarea' : data?.demoType === 'input' ? 'input' : null;
  const title = demoType === 'textarea' ? 'Textarea' : 'Input';
  const subtitle = demoType === 'textarea' ? 'Native textarea behavior' : 'Native input behavior';

  if (!demoType) {
    return <div className="min-h-screen bg-gray-100" />;
  }

  return (
    <div className="min-h-screen bg-gray-100">
      <div className="px-4 py-5 space-y-4">
        <header className="bg-gradient-to-r from-blue-500 to-cyan-600 rounded-xl px-4 py-4">
          <div className="text-lg text-white font-bold">{title}</div>
          <div className="text-xs text-white/85 mt-1">{subtitle}</div>
        </header>

        {demoType === 'textarea' ? <TextareaDemo data={data} /> : <InputDemo data={data} />}
      </div>
    </div>
  );
}
