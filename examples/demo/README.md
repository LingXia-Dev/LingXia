# LingXia Demo Apps

## 🚀 Getting Started

```bash
cd examples/demo/homelxapp
npm install
npm run build:dev
ls -la dist/
```

## 📱 Apps

### `homeminiapp`
Multi-framework demo app with HTML, Vue, and React pages.

## 🏗️ Page Types

### HTML Pages (`*.html`)
Static content with direct HTML/CSS/JS. No compilation needed.

### Vue SPA (`*.vue`)
Interactive pages with Vue 3. Compiled with Vite + asset inlining.

### React SPA (`*.tsx`)
Component-based pages with React 18. (Coming soon)

## 📁 Structure

```
homelxapp/
├── app.json              # App config (lxAppId, lxAppName, version required)
├── app.js                # App entry logic
├── pages/                # Pages
│   ├── home/
│   │   ├── index.html    # HTML page
│   │   └── index.js      # Page logic
│   ├── API/index.html    # HTML page
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

### Logic Layer (JSCore/QuickJS)
- `app.js` + `pages/*/index.js` → `logic.js`
- Page functions defined in `Page({})` object
- Handles business logic and native API calls

### View Layer (WebView)
- HTML/Vue/React pages for UI rendering
- Communicates with logic layer via bridge
- Access to page JS functions
