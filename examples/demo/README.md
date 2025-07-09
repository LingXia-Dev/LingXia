# LingXia Demo App

## 🚀 Getting Started

```bash
cd examples/demo/homelxapp
npm install
npm run build:dev
ls -la dist/
```

## LxApp

### `homelxapp`
Multi-framework demo app with HTML, Vue, and React pages.

## 🏗️ Page Types

### HTML Pages (`*.html`)
Static content with direct HTML/CSS/JS. No compilation needed.

### Vue SPA (`*.vue`)
Interactive pages with Vue 3. Compiled with Vite + asset inlining.

### React SPA (`*.tsx`)
Component-based pages with React 18.

## 📁 Structure

```
homelxapp/
├── lxapp.json              # LxApp config (lxAppId, lxAppName, version required)
├── lxapp.js                # LxApp entry logic
├── pages/                # Pages
│   ├── home/
│   │   ├── index.html    # HTML page
│   │   └── index.js      # Page logic
│   ├── API/index.tsx     # React SPA
│   └── todo/index.vue    # Vue SPA
├── images/               # Static assets (images/, assets/, static/, public/)
└── dist/                 # Build output
```

## 🔨 Commands

```bash
npm run build:dev        # Development build
npm run build:prod       # Production build
```

## 🔧 Architecture

### Logic Layer (JSCore, QuickJS etc)
- `lxapp.js` + `pages/*/index.js` → `logic.js`
- Page functions defined in `Page({})` object
- Handles business logic and native API calls

### View Layer (WebView)
- HTML/Vue/React pages for UI rendering
- Communicates with logic layer via bridge
- Access to page JS functions
