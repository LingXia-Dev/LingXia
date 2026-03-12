# @lingxia/components

Pre-built UI component library for LingXia lightweight applications.

## Directory Structure

```
lingxia-components/
├── src/          # TypeScript source files
│   ├── react/   # React components
│   ├── vue/     # Vue components
│   └── *.ts     # Vanilla JS / Custom Elements
└── dist/         # Built bundles (generated)
```

## Usage

### React

```tsx
import { LxVideo, LxPicker, LxNavigator } from '@lingxia/components/react';

function App() {
  return (
    <div>
      <LxVideo
        src="https://example.com/video.mp4"
        controls
        onPlay={(e) => console.log('Playing', e)}
      />

      <LxPicker
        columns={[['Option 1', 'Option 2', 'Option 3']]}
        value="Option 1"
        onConfirm={(value) => console.log('Selected:', value)}
      />

      <LxNavigator url="/other-page" openType="navigate">
        Go to other page
      </LxNavigator>
    </div>
  );
}
```

### Vue

```vue
<script setup>
import { LxVideo, LxPicker, LxNavigator } from '@lingxia/components/vue';
import { ref } from 'vue';

const selectedValue = ref('Option 1');
</script>

<template>
  <div>
    <LxVideo
      src="https://example.com/video.mp4"
      controls
      @play="(e) => console.log('Playing', e)"
    />

    <LxPicker
      :columns="[['Option 1', 'Option 2', 'Option 3']]"
      v-model="selectedValue"
      @confirm="(value) => console.log('Selected:', value)"
    />

    <LxNavigator url="/other-page" open-type="navigate">
      Go to other page
    </LxNavigator>
  </div>
</template>
```

### Vanilla JS / Custom Elements

```javascript
import { registerVideoComponent, registerPickerComponent } from '@lingxia/components';

// Register custom elements
registerVideoComponent();
registerPickerComponent();

// Use in HTML
// <lx-video src="video.mp4" controls></lx-video>
// <lx-picker columns='[["A","B","C"]]'></lx-picker>
```

## Components

| Component | Description |
|-----------|-------------|
| `LxVideo` | Native video player with quality switching, playback rate control |
| `LxPicker` | Native picker for selector, multi-selector, cascading, date, and time |
| `LxNavigator` | Navigation component for page navigation, external links, phone calls |

## Development

```bash
npm install
npm run build
```

## Design Docs

- [Native Components Design and API Contract](./docs/native-components-design.md)
- [Component API Reference](./docs/component-api-reference.md)
