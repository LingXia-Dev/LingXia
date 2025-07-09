# LingXia Builder

Build tool for LingXia App development. Supports HTML, Vue, and React pages.

## Features

- **Multi-framework**: HTML, Vue (.vue), React (.tsx/.jsx)
- **Function injection**: Automatic page function registration
- **Asset bundling**: CSS, images, and JSON files
- **Unified build**: Single logic.js with all page functions

## Usage

Add to your `vite.config.js`:

```javascript
import { lingxiaPlugin } from './lingxia-builder/vite-plugin.js';

export default {
  plugins: [lingxiaPlugin()],
  build: {
    rollupOptions: {
      input: 'lxapp.js',
      output: {
        format: 'es',
        entryFileNames: '[name].js',
        dir: '.lingxia-build/main-app'
      }
    }
  }
};
```

## Project Structure

```
my-miniapp/
├── lxapp.js                   # Application entry
├── lxapp.json                 # App configuration
├── pages/
│   ├── home/
│   │   ├── index.html       # HTML page
│   │   ├── index.js         # Page functions
│   │   └── index.css        # Page styles
│   ├── profile/
│   │   ├── index.vue        # Vue page
│   │   ├── index.js         # Page functions
│   │   └── index.css        # Page styles
│   └── settings/
│       ├── index.tsx        # React page
│       ├── index.js         # Page functions
│       └── index.css        # Page styles
└── images/                  # Static assets
```

## Page Types

- **HTML**: Static pages with optional JavaScript functions
- **Vue**: Single-file components (.vue) with Vite build
- **React**: TSX/JSX components (.tsx/.jsx) with Vite build

## Build Output

```
dist/
├── pages/
│   ├── home/index.html      # Built HTML page
│   ├── profile/index.vue    # Built Vue page
│   └── settings/index.tsx   # Built React page
├── logic.js                 # Combined page functions
├── lxapp.json                 # App configuration
└── images/                  # Static assets
```
