# React showcase → GPU parity

Every page of `../lingxia-showcase` except the native-component gallery has a
GPU View here, built on `lxapp/shared/ui` and the original Logic. Status is per
page; "device" means checked on a physical Android device, "CPU" means compiled
and rendered by the shared core off-device.

Legend: **D** device-verified behavior from earlier GPU builds · **C** compiled,
executed and CPU-rendered after the CSS-named authoring rewrite · — not verified.

| Page | Workflows | Earlier device evidence | After rewrite | Open on device |
| --- | --- | --- | --- | --- |
| Home | greeting, appearance, language, version, IP | D | C | backdrop blur pixels, app links |
| ToDo | add, toggle, filters, delete, clear, persistence | D | C | List virtualization, storage failure UI |
| API index | grouped navigation to every API page | D | C | all routes after rewrite |
| Device | device/screen/network info, listener, orientation, dial, vibrate | D | C | phone/vibration, denied permissions |
| System | base info, settings, autostart, cache, notifications, banner | D | C | notification targets, banner (desktop only) |
| Wi-Fi | module, scan, listener, connect | D | C | real connection, denied permissions |
| Location | fix, error, clear | D (error path) | C | successful fix |
| Clipboard | text, typed, image, inspect, clear | D | C | failure paths, keyboard |
| Share | text, page link, image, file | D | C | App Link association |
| File | PDF download/pause, Office fetch, chooser | D | C | Office handler, timeouts |
| UI APIs | navigation stack, toast, modal, sheet, navbar, tabbar, appearance, surface | D | C | popup Back arbitration, rotation |
| Pull to refresh | gesture and API start/stop | D | C | system gesture cancellation |
| Leave guard | Back/Home interception, keep/discard | D | C | rotation while asking |
| Feedback | form, category, submit | D | C | failures |
| Cloud | auth, tenants, MQTT, functions | D (provider build) | C | reconnect, permission failures |
| Media | pictures, videos, scan, image/video tools, save to album | D | C | camera QR, album writes |
| Stream | tokens, charts, cancel, clear | D | C | chart animation |
| Channel | ticker subscribe, disconnect, reconnect | D | C | error presentation |
| Bridge repro | bootstrap, echo, gap-free ticks | D | C | transport replacement stress |
| Surface | child page lifecycle, messages, self hide/close | D | C | multiple surfaces |
| Components, video, picker, navigator, swiper | — | excluded | excluded | native-component demos are out of scope |

## Cross-cutting

| Area | State |
| --- | --- |
| Design tokens | `lxapp/theme/tokens.css` matches the React reference (`theme-parity.test.mjs`) |
| Host navigation config | page JSON and tabs match the reference (`navigation-parity.test.mjs`) |
| API catalog | entries and order match the reference (`api-catalog-parity.test.mjs`) |
| Charts | percentages and emphasis colors match recorded ECharts/zrender values |
| Accessibility | one node per composed control, named by its text; screen-reader speech unverified |
| Hover | not exercised: Android reports a coarse pointer |
| Animation | not authored; see `docs/internal/ui-animation.md` |

Parity tests compare against the React showcase as committed at `HEAD`; uncommitted
edits there print a notice naming the file and are not compared until committed.

Renderer probes and fixtures that used to live in a `lab` page are now the
Logic-free fixture app in `lingxia-sdk/android/tests/gpu/fixtures/renderer`.
