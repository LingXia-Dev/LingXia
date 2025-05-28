# LingXia MiniApp Builder

Official build tool for LingXia MiniApp development. Combines individual page files into optimized packages with unified logic.js generation and asset bundling.

## 🚀 Features

- **Unified Logic.js**: Combines all page JavaScript files into a single optimized file
- **Asset Bundling**: Automatically packages images, styles, and layouts
- **JSON Validation**: Validates all JSON files for syntax errors before building
- **Code Optimization**: Minification and comment removal for production builds
- **ZIP Packaging**: Creates ready-to-deploy MiniApp packages
- **Vite Integration**: Modern build system with plugin architecture

## 📦 Installation

```bash
npm install @lingxia/miniapp-builder --save-dev
```

## 🔧 Usage

### Vite Plugin (Recommended)

```javascript
// vite.config.js
import { defineConfig } from 'vite';
import { LingXiaMiniAppBuilder } from '@lingxia/miniapp-builder/vite';

export default defineConfig({
  plugins: [
    LingXiaMiniAppBuilder({
      minifyCode: process.env.NODE_ENV === 'production',
      removeComments: process.env.NODE_ENV === 'production',
      createPackage: process.env.NODE_ENV === 'production'
    })
  ],

  build: {
    rollupOptions: {
      input: 'app.js',
      external: () => false,
      output: {
        format: 'es',
        entryFileNames: 'temp.js'
      }
    }
  }
});
```

### CLI Usage

```bash
# Basic build
npx @lingxia/miniapp-builder

# With options
npx @lingxia/miniapp-builder --minifyCode true --createPackage true --packageName my-app.zip
```

### Programmatic API

```javascript
import { buildMiniApp } from '@lingxia/miniapp-builder';

await buildMiniApp({
  minifyCode: true,
  removeComments: true,
  createPackage: true,
  assetDirs: ['images', 'assets']
});
```

## ⚙️ Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `configFile` | string | `'app.json'` | MiniApp configuration file |
| `projectConfigFile` | string | `'project.config.json'` | Project configuration file |
| `outputFile` | string | `'logic.js'` | Generated logic file name |
| `buildDir` | string | `'dist'` | Build output directory |
| `assetDirs` | array | `['images']` | Asset directories to copy |
| `minifyCode` | boolean | `false` | Enable code minification |
| `removeComments` | boolean | `false` | Remove comments from output |
| `createPackage` | boolean | `false` | Create ZIP package |
| `packageName` | string | `null` | ZIP package filename (uses appId if not set) |
| `targetDir` | string | `null` | Target directory for host app assets |
| `copyToTarget` | boolean | `false` | Copy build to target directory |

## 📁 Project Structure

Your MiniApp project should follow this structure:

```
my-miniapp/
├── app.js                 # Application entry point
├── app.json               # Application configuration
├── project.config.json    # Project configuration
├── pages/
│   ├── home/
│   │   ├── index.js       # Page logic
│   │   ├── index.html     # Page layout
│   │   ├── index.css      # Page styles
│   │   └── index.json     # Page configuration
│   └── about/
│       ├── index.js
│       ├── index.html
│       ├── index.css
│       └── index.json
├── common/                # Shared utilities (optional)
│   └── utils.js
├── images/                # Static assets
│   └── logo.png
└── vite.config.js         # Build configuration
```

## 📝 Configuration

### project.config.json (Required)
Simple project configuration with just the essentials:

```json
{
  "appid": "com.example.my-miniapp",
  "projectname": "MyMiniApp"
}
```

### app.json
Define your pages and app configuration:

```json
{
  "pages": [
    "pages/home/index.html",
    "pages/API/index.html",
    "pages/todo/index.html"
  ],
  "debug": true,
  "tabBar": {
    "color": "#999999",
    "selectedColor": "#1677ff",
    "backgroundColor": "#ffffff",
    "borderStyle": "#eeeeee",
    "position": "bottom",
    "list": [
      {
        "text": "Home",
        "pagePath": "pages/home/index.html",
        "iconPath": "images/home.png",
        "selectedIconPath": "images/home_selected.png",
        "selected": true
      },
      {
        "text": "API",
        "pagePath": "pages/API/index.html",
        "iconPath": "images/api.png"
      },
      {
        "text": "ToDo",
        "pagePath": "pages/todo/index.html",
        "iconPath": "images/todo.png",
        "selectedIconPath": "images/todo_selected.png"
      }
    ]
  }
}
```

### vite.config.js
Configure the build tool:

```javascript
import { defineConfig } from 'vite';
import { LingXiaMiniAppBuilder } from '@lingxia/miniapp-builder/vite';

export default defineConfig({
  plugins: [
    LingXiaMiniAppBuilder({
      minifyCode: process.env.NODE_ENV === 'production',
      removeComments: process.env.NODE_ENV === 'production',
      createPackage: process.env.NODE_ENV === 'production'
    })
  ],

  build: {
    rollupOptions: {
      input: 'app.js',
      external: () => false,
      output: {
        format: 'es',
        entryFileNames: 'temp.js'
      }
    }
  }
});
```

## 🔄 Development Workflow

1. **Setup**: Install the builder and configure your `vite.config.js`
2. **Configure**: Create `project.config.json` with your appId
3. **Develop**: Write your pages using standard `Page()` calls
4. **Build**: Run `npm run build` to generate the package
5. **Deploy**: Use the generated ZIP file or dist folder

## 📦 Build Output

The builder generates a `dist/` directory with:

```
dist/
├── logic.js              # Unified App + Pages logic (dependencies bundled)
├── app.json              # Application configuration
├── project.config.json   # Project configuration
├── pages/                # Page assets (HTML, CSS, JSON)
└── images/               # Static assets
```

## 🎯 Page Registration

The builder automatically transforms your `Page()` calls:

**Before (your code):**
```javascript
// pages/home/index.js
Page({
  data: { message: 'Hello' },
  onLoad() { console.log('Page loaded'); }
});
```

**After (generated logic.js):**
```javascript
Page({
  data: { message: 'Hello' },
  onLoad() { console.log('Page loaded'); }
}, "pages/home/index");
```

## 🛠️ Shared Dependencies

The builder automatically bundles all dependencies into `logic.js`. Create shared utilities anywhere in your project and import them:

```javascript
// utils/helpers.js
export function formatDate(date) {
  return date.toLocaleDateString();
}

export function generateId() {
  return Date.now().toString(36);
}
```

Use them in your pages:

```javascript
// pages/todo/index.js
import { formatDate, generateId } from '../../utils/helpers.js';

Page({
  addTodo() {
    const todo = {
      id: generateId(),
      createdAt: formatDate(new Date())
    };
    // ...
  }
});
```

All dependencies are automatically resolved and bundled into the final `logic.js` file.

## 📋 Build Scripts

Add these scripts to your `package.json`:

```json
{
  "scripts": {
    "build": "vite build",
    "build:dev": "NODE_ENV=development vite build",
    "build:prod": "NODE_ENV=production vite build"
  }
}
```

## 🚀 Production Builds

Production builds automatically:
- ✅ Minify JavaScript code
- ✅ Remove all comments
- ✅ Create ZIP packages
- ✅ Optimize file sizes

```bash
npm run build:prod
```

## 🔧 Development Builds

Development builds preserve:
- ✅ Readable code formatting
- ✅ Comments and documentation
- ✅ Debug information

```bash
npm run build:dev
```

## 📄 License

MIT License - see LICENSE file for details.
