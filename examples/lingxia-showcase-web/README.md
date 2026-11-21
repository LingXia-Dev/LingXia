# LingXia Showcase (WebView)

A standalone lxapp with React pages. It shows what only a WebView page can:
native components inside a page (picker, media swiper, navigator), the page's
own `<video>`, the bridge (a chat stream, a two-way channel, a bridge under
load), and native pull to refresh. It has no native host; it runs in the
LingXia Runner.

## Run

```bash
npm install
lingxia dev -p macos          # or android, ios, harmony, windows
```

Every page has its shared types in `pages/<page>/contract.ts`; `index.ts` is
the page's Logic and `index.tsx` its View.

## Test

With the `lingxia dev` session running, from this directory:

```bash
lxdev test tests/
```

Two groups need a fixture and skip without it:

```bash
node tests/harness/http-fixture.mjs --port 0          # prints the base URL
lxdev test tests/pages/embedded-document.test.ts tests/pages/media-swiper-video.test.ts \
  --arg httpBase=http://127.0.0.1:<port>
```

On Android the app reaches the HTTP fixture through `adb reverse tcp:<port> tcp:<port>`.
The specs in `tests/platform/android/` drive the phone through the input fixture:

```bash
node tests/harness/android-device-fixture.mjs --device <serial>   # prints its URL
lxdev test tests/platform/android/ --arg platform=android --secret-arg androidDevice=<printed URL>
```
