# LingXia Builder

A modern, framework-agnostic build tool for LingXia Application development. Supports HTML, Vue, and React pages with automatic function bridging and optimized builds.

## Features

- **Multi-framework Support**: HTML, Vue (.vue), React (.tsx/.jsx)
- **Automatic Function Bridging**: Page functions automatically registered for native calls
- **Asset Optimization**: CSS merging, image copying, and resource bundling
- **Production Ready**: Code minification, tree shaking, and optimization
- **Template-driven**: Easy to extend with new frameworks
- **TypeScript Support**: Full TypeScript support for React pages

## Installation

### Global Installation (Recommended)

```bash
npm install -g lingxia-builder
```

## Usage

### Command Line Interface

```bash
# Development build
lingxia build

# Production build (with minification and optimization)
lingxia build --prod

# Create new project
lingxia create my-app

# Show help
lingxia --help
```



## Project Structure

```
my-miniapp/
├── lxapp.json                 # App configuration
├── pages/
│   ├── home/
│   │   ├── index.html       # HTML page
│   │   ├── index.js         # Page functions (logic layer)
│   │   ├── index.json       # Page configuration
│   │   └── index.css        # Page styles
│   ├── profile/
│   │   ├── index.vue        # Vue SPA page
│   │   ├── index.js         # Page functions (logic layer)
│   │   ├── index.json       # Page configuration
│   │   └── index.css        # Page styles
│   └── api/
│       ├── index.tsx        # React page
│       ├── index.js         # Page functions (logic layer)
│       ├── index.json       # Page configuration
│       └── index.css        # Page styles
├── src/                     # Shared components (for React/Vue)
├── images/                  # Static assets
└── lxapp.css               # Global styles
```

## Page Types

### HTML Pages
- Static HTML with optional JavaScript functions
- Direct rendering, no build step required
- Perfect for simple pages and landing pages

### Vue Pages (.vue)
- Single-file Vue components
- Built with Vite for optimal performance
- Supports Vue 3.5+ features and composition API

### React Pages (.tsx/.jsx)
- React components with TypeScript support
- Built with Vite and React 19
- Supports modern React features and hooks

## Build Modes

### Development Build
```bash
lingxia build --dev
```
- Fast builds with source maps
- No minification for easier debugging
- Detailed build logs

### Production Build
```bash
lingxia build --prod
```
- Code minification and optimization
- Tree shaking for smaller bundles
- Asset optimization and compression

## Build Output

```
dist/
├── pages/
│   ├── home/
│   │   ├── index.html       # Built HTML page
│   │   ├── index.css        # Merged styles
│   │   └── index.json       # Page config
│   ├── profile/
│   │   ├── index.vue        # Built Vue page
│   │   ├── view.js          # Compiled Vue component
│   │   ├── index.css        # Merged styles
│   │   └── index.json       # Page config
│   └── api/
│       ├── index.tsx        # Built React page
│       ├── view.js          # Compiled React component
│       ├── index.css        # Merged styles
│       └── index.json       # Page config
├── logic.js                 # Combined page functions (logic layer)
├── lxapp.json              # App configuration
├── lxapp.css               # Global styles
└── images/                 # Static assets
```



## Advanced Features

### Function Bridging
Page functions defined in `index.js` are automatically bridged to the view layer:

```javascript
// pages/home/index.js
Page({
  greet: function(name) {
    return `Hello, ${name}!`;
  },

  getData: async function() {
    return await lx.request('/api/data');
  }
});
```

These functions become available in your view components and can be called from native code.

### CSS Merging
- Original page CSS is preserved
- Framework-generated CSS is appended
- Global `lxapp.css` is injected at runtime

### Asset Optimization
- Images are copied to dist with original structure
- CSS imports are resolved and bundled
- Static assets maintain relative paths

## Troubleshooting

### Common Issues

1. **Page functions not working**: Ensure logic file matches page file name
2. **CSS not loading**: Check that CSS files exist and paths are correct
3. **Build failures**: Verify all dependencies are installed

### Debug Mode
```bash
lingxia build --dev --verbose
```

## Contributing

LingXia Builder uses a template-driven architecture that makes it easy to add new frameworks. See the `src/templates/` directory for examples.

## License

MIT
