# lingxia-components

Pre-built UI component library for LingXia lightweight applications.

## Directory Structure

```
lingxia-components/
├── src/          # TypeScript source files
│   ├── react/   # React components
│   ├── vue/     # Vue components
│   └── *.ts     # Vanilla JS components
└── dist/         # Built bundles (generated)
```

## Usage

```javascript
// Vanilla JS
import { Component } from 'lx://view/component.js';

// React
import { Button } from 'lx://view/react/Button.js';

// Vue
import { Button } from 'lx://view/vue/Button.js';
```

## Development

```bash
npm install
npm run build
```
