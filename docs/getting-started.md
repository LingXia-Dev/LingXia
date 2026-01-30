# Getting Started

Welcome to LingXia! This guide will help you create your first LingXia project.

---

## Prerequisites

- **Node.js** 18 or later
- **Rust** (for projects with native code)
- **Android Studio** (for Android development)
- **Xcode** (for iOS development, macOS only)

Verify your environment:

```bash
lingxia doctor
```

---

## Installation

Install the LingXia CLI globally:

```bash
npm install -g @lingxia/cli
```

Verify installation:

```bash
lingxia --version
```

---

## Create a New Project

### Host App (Native App with LxApp Container)

Create a complete native app project:

```bash
lingxia new my-app
```

Interactive mode will prompt for:
- Project type (native-app or lxapp)
- Product name (defaults to project name)
- Target platforms (Android, iOS, Harmony)
- Package ID

Or use flags to skip prompts:

```bash
lingxia new my-app --platform android,ios --package-id com.example.myapp -y
```

### LxApp Only

Create a standalone LxApp:

```bash
lingxia new my-lxapp --project-type lxapp
```

---

## Project Structure

### Host App Project

```
my-app/
├── lingxia.config.json  # Host app configuration
├── .lingxia.secrets.json # Secrets (hidden, not tracked by git)
├── android/             # Android native project
├── ios/                 # iOS native project (if selected)
├── harmony/             # HarmonyOS project (if selected)
└── homelxapp/           # Embedded LxApp (defaults to app.homeLxAppID)
```

### LxApp Project

```
my-lxapp/
├── lxapp.json           # App metadata (lxAppId, lxAppName, version)
├── lxapp.config.json    # Build configuration
├── package.json         # NPM dependencies
├── pages/               # Page components
│   └── home/
│       ├── index.tsx    # View layer (React/Vue)
│       ├── index.ts     # Logic layer
│       └── index.json   # Page config
├── public/              # Static assets
└── shared/              # Shared utilities
```

---

## Development

### Run Development Mode

For Host App:

```bash
cd my-app
lingxia build
```

For LxApp only:

```bash
cd my-lxapp
lingxia build
```

### Build for Production

Host App:

```bash
lingxia build --prod
```

LxApp only:

```bash
lingxia build --prod
```

---

## Dependencies

### Version Management

LingXia uses **unified versioning**—all components share the same major.minor version.

Your `package.json` will include:

```json
{
  "dependencies": {
    "@lingxia/rong": "^0.1.1"
  }
}
```

### Check Environment

```bash
lingxia doctor
```

Shows status of:
- Java/JDK
- Android SDK
- Gradle
- Rust toolchain
- Android NDK

---

## Next Steps

- [CLI Command Reference](./cli.md) - All available commands
- [Bridge API Spec](./lingxia_bridge_spec.md) - Bridge API reference
- [App Links](./applinks.md) - Deep linking configuration
