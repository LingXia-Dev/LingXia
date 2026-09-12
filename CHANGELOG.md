# Changelog

From 0.13.0 on, generated from the commit log — a commit subject *is* its
changelog entry, so nothing here is written by hand. 0.12.0 is the exception:
it is where this file starts, so it says so rather than restating the history
behind it. Regenerate the pending section any time:

```bash
scripts/release/main.sh changelog
```

LingXia is `0.x`. Anything may change in a minor release; a change that breaks
callers is listed under **Breaking** with the release that made it.

Entries are grouped by who a change lands on, not by commit type — the same fix
means different things to someone writing an lxapp and someone embedding the
SDK. Changes worth more than one line carry a `Release-Note:` trailer and are
written out in full in that release's notes on GitHub.

<!-- releases below -->

## 0.16.0 — 2026-09-12

### Breaking

- **Breaking** — split host env from lxapp channel (6bb7c8b90)
- **Breaking** — **process**: remove downstream authority minting (0db8f4b1e)
- **Breaking** — **security**: seal platform control authority (9d516ed7c)
- **Breaking** — **logic**: authorize control operations before argument decoding (b08ffd1e5)
- **Breaking** — **process**: bind execution to live session authority (156652879)
- **Breaking** — **terminal**: seal native automation authority (ad528e77b)
- **Breaking** — **config**: require browser control protocol v3 (eba7b2bd9)
- **Breaking** — **logic**: remove host-wide Settings APIs (39cdb6759)
- **Breaking** — **bridge**: unify effective route inventory (f4c0d4209)
- **Breaking** — **security**: require native grants for privileged resources (4e9912c18)
- **Breaking** — **settings**: migrate display language consumers (e5224ac3a)
- **Breaking** — **browser**: enforce document-bound control v3 (7b4afa0ce)
- **Breaking** — **bridge**: classify native route audiences (ad386f165)

### Writing an lxapp

- **update**: keep UpdatePackageInfo fixtures compiling after rebase (1ba9a34e8)
- **lxapp**: verify check-update envelopes before download (5ff3070b7)
- **update**: sign publishes and verify check-update envelopes (dece61799)
- **lxapp**: update developer packages by checksum (42f8e749f)
- **theme**: let a host choose the appearance a product starts in (a0fb6030b)
- **bridge**: never replay a replaced document's Apple downstream frames (8d7840538)
- **bridge**: reconnect an Apple downstream that never connects (d9882f06e)
- **webview**: let a success that finished before its commit still bind (022a522ab)
- **logic**: let previewMedia accept https image and video URLs (871579f61)
- **lxapp**: reject tab bar commits that native actually failed (8182ba883)
- **lxapp**: re-resolve Auto appearance when reopening a guest (ca011eec4)
- **lxapp**: keep chrome patches when native paint is deferred (97cf361b1)
- **tabbar**: look up live lxapps when rebuilding overflow chrome (b6175c7be)
- **tabbar**: rebuild tab chrome off the JS worker (1445f3c74)
- **tabbar**: follow display language for overflow More (b232ba79f)
- **lxapp**: keep the current tab selected after in-place reload (bf6ea942c)
- **lxapp**: rebuild tabBar and navBar chrome on reload (4c7977cd2)
- **lxapp**: apply pages, tabBar, and navigationStyle on reload (0fa64bc15)
- **tabbar**: stop racing pad overflow tests and pass JNI bool through (47eb27486)
- **tabbar**: show up to ten items on pad strips (d7bc32c20)
- **lxapp**: host-owned permissions on the registry record (5c14cd467)
- **logic**: name an aborted transfer or preview `AbortError` (ed801a800)
- **lxapp**: hide scrollbars on lxapp pages (776fb9e65)
- **test**: repair Windows automation for appearance and terminal settings (b732ca49c)
- **bridge**: keep bridge diagnostics off the page console wrapper (cff0b8a68)
- **logic**: decide each control route's audience explicitly (ec2df9ec6)
- **lxapp**: never dispatch a bound-V3 frame under its owner's identity (2be528fad)
- **lxapp**: keep a saved language when the system locale will not parse (7327f504d)
- **lxapp**: derive the ControlApp class from the home identity (9de327322)
- **webview**: release the normalizer locks before running inbound code (4faba478d)
- **logic**: name the class a denied route wanted, and report a catalog gap once (4950016de)
- **lxapp**: give the display language one namespace and one writer (9389b294e)
- **vue**: write native island aria-label as a DOM attribute (3f5f08421)
- **test**: catalog lx.app.cache as a namespace; pin tinyvec 1.12.0 (f6bec457e)
- **elements**: keep layout labels from painting over native text (583f70032)
- **elements**: align native components with dynamic page rendering (3920a0823)
- **elements**: set island video playing from autoplay on the leaf (54eb2571e)
- **bridge**: do not rebind the native receiver on duplicate init (719be074e)
- **lxapp**: prefix unused island apply root (09b6256fa)
- **elements**: emit an update when island text changes (77ffdaf03)
- **lxapp**: accept an unmount the parent's cascade already took (d4298736b)
- **elements**: honour controls="false" on the legacy video path (f3436bff9)
- **elements**: give every island node a measured rect (a67404760)
- **elements**: retry an island commit the host never received (e6dd9ba4e)
- **elements**: commit the island tree while the document is hidden (f92b1f7d8)
- **html**: lock inline native runtime dependency (43eec7478)
- **lxapp**: overlay latched slider value onto island paint props (5fd837597)
- **webview**: consume island pointer hits on the WebView surface (16aedc2e6)
- **elements**: dispatch host press and slider events on author elements (2179c8bac)
- **lxapp**: paint Cover/Button/Slider and hit-test from the island session (e01c459bf)
- **webview**: present island video frames on the shared DComp tree (3ca4f831f)
- **webview**: stage island visuals on the existing geometry commit (cd43b0eb3)
- **webview**: attach island visuals above the WebView DComp plane (38fc8203f)
- **lxapp**: drive island paint through IslandCompositor (11e2058c4)
- **lxapp**: apply a remounted base-0 commit over a stale island slot (fc2e2f267)
- **lxapp**: expose composition_nodes for island paint factories (cbfd9e35e)
- **elements**: include LxNativeText content in root commits (f3b9f5422)
- **bridge**: send native-component messages through Harmony LingXiaProxy (6f48ce3fc)
- **lxapp**: complete island lease handshake and apply trustedDomains (fc39a1873)
- **elements**: emit live root commits, geometry snapshots, and lease accept (f85f9c363)
- **lxapp**: satisfy clippy on the inline native protocol (fa756540b)
- **lxapp**: validate inline native media URLs against trustedDomains (704caee4b)
- **lxapp**: host inline native composition on all platforms (7cf5bbd8a)
- **elements**: move LxVideo onto the inline native island (023ec89d5)
- **lxapp**: add inline native root protocol applicator (babefa0ea)
- **elements**: add inline native author components and wrappers (736706df6)
- **lxapp**: give Terminal Settings its own ControlSurface class (2985dbb9c)
- **webview**: avoid bound message UI deadlock (6fef3dcd3)
- **lxapp**: keep same-route redirect document live (e15b3a874)
- **lxapp**: dispatch warm reload before entry resumes (2feacbbac)
- **logic**: activate newly opened main apps (7536b03dc)
- **logic**: report context capabilities accurately (ed695acc7)
- **webview**: cap native message adapters (8de8392ac)
- **webview**: prevent NavigationId reuse (3a8ba5e5c)
- **runtime**: resolve settings destinations fresh (28d307366)
- **bridge**: gate document-bound work and delivery (bed5f5cf1)
- **bridge**: support dormant document-bound v3 (bb2fe648b)
- **webview**: bind messages to document generations (9bce6a274)
- **bridge**: add dormant v3 codec (c9bd944e4)
- **lxapp**: route AppLinks from query, deliver scene 8003 once (740d500d1)
- **lxapp**: keep home identity and region across OTA apply (8551c49cd)

### Embedding a host app

- **windows**: host exclusive tray as a notification-area flyout (e2faf3051)
- **windows**: rasterize SVG tray glyphs and invert them on a dark taskbar (10049c91f)
- **apple**: link standalone Rust tests with host stubs (10f8e0465)
- **android**: pin Camera/Media3/core-ktx to compileSdk 35 (a25b63738)
- **android**: share SDK catalog aliases with the showcase host (c6ab0c419)
- **android**: allow hosts to replace the URL playback engine (d196f3fc0)
- **android**: keep the live app on screen across launcher taps and links (9232c1df3)
- **harmony**: commit a visible document under the URL its begin reported (8dbfc7896)
- **android**: keep a strict page's early port request across its load start (8e2fcfbc4)
- **harmony**: route https Wants through AppLink and allow host takeover (9bc8cb736)
- **android**: let hosts take over inbound links and deliver cold links after init (78935456c)
- **harmony**: guard the onPageEnd port fallback and defer a racing getPort (9aa19ddbe)
- **android**: guard the finished-load port fallback and re-mint a repeated getPort (b2707ac80)
- **ios**: keep layer.render for WebView, overlay AVPlayer frames (a959b3dd3)
- **ios**: capture AVPlayer frames in app screenshots (0e8b4366c)
- **harmony**: cancel delayed player destroy when a new preview opens (8f1275d3b)
- **ios**: recover playback audio session without invalid options (59d573563)
- **harmony**: settle https preview images that fail to load (c11892d43)
- **macos**: warn when a preview gallery skips an unreachable item (ac49d5c22)
- **macos**: drop unreachable preview items and keep request indexes (1ee3183a0)
- **ios**: fetch remote preview images off the main thread (9a5b33c82)
- **media**: keep https preview sources remote on every host (28dc4818c)
- **harmony**: tint shared capsule icons with SRC_IN (bd2fc50de)
- **harmony**: resync home language on guest close and enlarge capsule icons (015a890b1)
- **android**: attach MessagePort when commit-visible is skipped (0c5f122b9)
- **harmony**: attach MessagePort when onPageVisible is skipped (58a7bb75a)
- **harmony**: remount tab items when rust patches text (42e9d79d6)
- **macos**: refresh tab bar items after in-place reload (93535237c)
- **platform**: report the canonical OS label on unsupported hosts (194334db6)
- **windows**: follow runner moon/sun in lx.appearance Auto (8f19ccc55)
- **windows**: drop publish=false now that windows-rs is crates.io (a2f5698fa)
- **windows**: hide overflow-tab parked parents as a second silhouette (7b7ef7ec0)
- **windows**: dismiss overflow on strip tab selection (022ee19fc)
- **windows**: align compact TabBar overflow with iOS (dc8549c34)
- **windows**: apply Settings appearance and language on live documents (1c31d0a59)
- **windows**: re-export host color-mode handler from lingxia-platform (b4b24d71f)
- **windows**: draw shell chrome in the product's scheme, not the OS's (d2d275917)
- **macos**: resolve shell chrome colours against the view, not the system (54596f65d)
- **app**: make light/dark one product setting, shaped like the language (e33069769)
- **app**: give every document the product language through the bridge (f1703b0fa)
- **macos**: clip overflow entrance above the simulated tab bar (5ddff50ed)
- **macos**: align overflow tabs left and preserve label width (2c0385a1d)
- **windows**: type video opacity channel arithmetic explicitly (042eb9124)
- **windows**: initialize UI Automation on each worker thread (835d684cf)
- **app**: preserve live lxapp caches during product cleanup (9566e4a05)
- **app**: sweep orphaned installs and staged archives on clear (8c763d972)
- **app**: add lx.app.cache for a product-wide clear-cache control (ac43759ae)
- **macos**: make page delta scrolling deterministic (46100a702)
- **windows**: do not fail CI on video notify play JS races (6456fdf78)
- **windows**: queue WebView2 native events until the bridge receiver exists (a990b2a72)
- **windows**: emit island playing when autoplay beats the view handshake (4d3b68d3e)
- **windows**: queue island video events until the view is ready (7d89ab0cf)
- **windows**: re-deliver island video playing events (be0eb0b7f)
- **apple**: negotiate the island lease per root (bc11bcb45)
- **macos**: correct island video routing, lifecycle and text fidelity (d372a09a0)
- **ios**: make the inline native island host work on device (0d515412b)
- **ios**: keep island video off the legacy scroll-container helper (5746ef2da)
- **windows**: drain island applies per page (7a4eae2f2)
- **windows**: preserve pending island playback (e137b99bc)
- **apple**: isolate island node UI state (64791131c)
- **android**: paint island video at the CSS rect and deliver host events (6cd207b7f)
- **harmony**: apply island root.commit update patches (80110e18b)
- **macos**: pass Cover box-none hits through to siblings below (8a43cd8b0)
- **windows**: rematerialize island slider from the latched drag value (ed9027f4b)
- **harmony**: factory island kinds and forward video commands (9892bf3c2)
- **macos**: factory island kinds, apply geometry, and emit host events (2c6988279)
- **ios**: factory island kinds, apply geometry, and emit host events (0f4bd4758)
- **android**: factory island kinds, apply geometry, and forward video commands (93a861832)
- **windows**: paint full-rect island kinds and route pointer events (435cec71c)
- **windows**: use clamp for island texture bounds (1d72c2415)
- **windows**: blit MFPlay frames onto the island DComp visual (f14a5e551)
- **windows**: apply island paint after the WebView message turn (38072ef53)
- **windows**: materialize island through compositor without HWND restack (ccc59e645)
- **windows**: factory ordered island nodes above the WebView surface (e1a08cdb9)
- **harmony**: apply island commit/geometry/lease and attach video NodeContainers (4bd2bbfad)
- **harmony**: factory island NodeControllers in committed order (1f44f1361)
- **windows**: materialize island video players after lease (0d3c976e9)
- **windows**: use a relative island video src in the host test (533215e39)
- **app**: add a product-wide light/dark appearance setting (8a9a0e8a5)
- **windows**: reject oversized web messages before the string copy (8beeaad9d)
- **windows**: gate runtime locale refresh by components (9188dde48)
- **macos**: present resolved settings browser tabs (6762c7280)
- **apple**: import the SDK module with its declared case (7c81819f7)
- **windows**: scope app browser pages to shell (8f70ec967)
- **macos**: validate trusted tab routes before opening (b8624be48)
- **windows**: navigate existing trusted browser tabs (86d555acb)
- **windows**: authorize fixed browser pages (e1e6ae07e)
- **windows**: present resolved static settings tabs (875007a2f)
- **windows**: scope browser-local routes to shell (02bd4985d)
- **macos**: authorize fixed browser routes (59269c7aa)
- **windows**: export static Settings shell source (f203c6c89)
- **macos**: project sealed Settings destination into chrome (179e86616)
- **browser**: decode escaped control console messages (dfbe4ea0e)
- **windows**: reprove restored control documents (98fd1d1f3)
- **android**: reprove restored control documents (f967416f9)
- **browser**: bind control document console ingress (d937bac5d)
- **browser**: reprove restored control documents (7eb5abc19)
- **harmony**: attest browser control documents (f28b788f9)
- **apple**: bind native components to lxapp pages (5c05ffce7)
- **apple**: revoke terminated document generations (a435a74f2)
- **browser**: rebootstrap restored control documents (352131f89)
- **android**: bind browser control to document ports (3f9774396)
- **browser**: harden control document ingress (78385f06d)
- **windows**: bind browser documents to WebView2 navigation (e0c26d5fd)
- **config**: add static settings destination schema (94c7fe151)
- **terminal**: bind settings and surfaces to native authority (642fa7b3b)
- **browser**: issue dormant control bootstraps (6b439b1a5)
- **browser**: attest trusted document loads (e3d642e9d)
- **macos**: sleep first-paint grace with nanoseconds (26d15f466)
- **macos**: hop page-transition paint wait on weak self (2fae49d46)

### Rust native extensions

- **automation**: pin Windows showcase lxdev to the session it started (304261c92)
- **native**: preserve dynamic composition and clipped rendering (bc3b1cc10)
- **native**: drop LxNativeSlider from the inline island (525dd2959)
- **native**: align island menus and scrolling (286986758)
- **native**: harden inline native island lifecycle (24cfbd394)
- **native**: preserve text and automation semantics (33c7f25a1)
- **native**: polish Apple and Harmony islands (330fe70f8)
- **native**: isolate Apple islands and release Harmony nodes (1080b38cc)
- **native**: harden inline component UX across hosts (5ee85584b)
- **automation**: install runtime for control apps (633706367)

### CLI and CI

- **cli**: sign publishes from --env (d69b100a2)
- **cli**: keep multi-line template literals intact in the logic bundle (763cecff8)
- **cli**: add the Universal Link entry to the macOS host template (a942be3c6)
- **cli**: drop the runner permissions warning (72a4d03a8)
- **cli**: auto-reload lxapps from lingxia dev (b42f6a99c)
- **cli**: generate the lxapp's page names as types (392489c1b)
- **cli**: rustfmt hide-scrollbar stamp helpers (a02df60e6)
- **cli**: keep an inline script out of the HTML whitespace pass (77e96ce57)
- **cli**: keep controlProtocolVersion off lingxia.yaml (17addea5c)
- **ci**: declare Apple static library to Cargo (9ab7dc525)
- **ci**: preserve full browser security suite (ccfd0c0bc)
- **ci**: run browser policy tests on Linux (492ba6348)
- **ci**: build Apple security test static library (236e28879)
- **release**: verify publishable workspace inventory (b8b23c39a)
- **lxdev**: inject App Links with app applink and lxapps.applink (5d092c18a)
- **cli**: reject CommonJS logic imports that declare no package type (af70b0dd7)
- **cli**: support per-env appLinks hosts (782b282fc)

### Docs and examples

- **showcase**: declare update trusted public keys (8bfa24217)
- **showcase**: add an opt-in Android libmpv URL engine (1e52e6c49)
- **showcase**: type-check display-language Logic helpers (30a52351e)
- **showcase**: reapply tab labels once native chrome exists (9f1b1cc7e)
- **showcase**: add home language switcher and page message catalogs (9ebecf4ea)
- **showcase**: follow display language for tab bar and home copy (d46a1ffae)
- **showcase**: drop host-injected cloud APIs from Logic coverage (f0fb56671)
- **showcase**: type-check the example's Logic (ff374753e)
- **types**: declare `sidebarActions` on the `ShellApi` `lx.shell` resolves to (3655d63f9)
- **types**: declare Storage.has (e231f4979)
- **types**: keep lazy-init data fields open and retire `Page<PageData>` (0bd587195)
- **types**: close the last three open surfaces (1f199bbc5)
- **types**: give Page and App configs their own shape (40ae88525)
- **types**: close obvious Logic typing footguns (7949841a3)
- **showcase**: keep native island UIA probe typed as a button node (3bd4c652e)
- **showcase**: scroll the document, not a wheel onto the island (85bf44834)
- **showcase**: narrow page query before reading native button rect (0ad16b0b2)
- **showcase**: stream native video demo online (faa3e348b)
- **showcase**: demonstrate native menu JS handoff (637f1d2ac)
- **showcase**: add H5-triggered native video menu (2e1610c77)
- **showcase**: demonstrate native cover and view (c326d56cb)
- **showcase**: match the DOM font weight to the island label (15916b8d9)
- **showcase**: size island Button/Slider anchors and query controls by id (311342dac)
- **showcase**: demo island Button and Slider beside the player (1f0e98a4f)
- **showcase**: play the local island sample so onPlaying can fire (2f9f9228e)
- **showcase**: require live island playback and ship a local sample (8aa9b851a)
- **showcase**: demo Root+Video+Cover and trust blender media hosts (6f5850bed)
- **showcase**: expose Vue UI lifecycle identity (964a5b9c6)
- **types**: ship real ESM behind the import condition (48d601349)

### Other

- **host**: preserve tray window behavior across SDK feature tiers (59f83ea77)
- **dev**: serialize runtime teardown and verify log session owners (f454b1d4f)
- **dev**: end desktop session when the runtime window closes (a7e98187e)
- **applink**: deliver any path on a configured host to home Logic (b98006ed5)
- **chrome**: follow display-language preference for tab and capsule labels (c8e5bac31)
- **chrome**: theme capsule overlay for dark and light (104453a6b)
- **chat**: follow display language for more-action labels (bc4204078)
- **android,harmony**: keep tabBar.update when chrome is not mounted (08302439d)
- **host**: look up capsule and tab More in display language (1758c718d)
- **chat**: type-check the example's Logic (1399d1a6e)
- **host**: load the saved appearance before the home lxapp exists (018e1b181)
- **host**: read the saved appearance after the runtime registry has a platform (0c6e730a2)
- **appearance**: release subscriber lock before querying native state (be8cc5140)
- **settings**: scope terminal language watch to ControlSurface (f135a0d06)
- **deps**: pin vendored rong-command to workspace 0.15.0 (0b1f2d10c)
- **host**: keep Process grants ControlApp-only at the authority (89edfe1fe)
- **devtools**: grant showcase automation authority (826990952)
- **rust**: scope authority facades to native consumers (9053dbda2)
- **process**: isolate Rong engines in workspace checks (a5c2b5991)
- **security**: seal native grant resolvers (70c7870cc)
- **security**: seal native control authority (e93c295f7)
- **settings**: restrict display language streams (c08624dc7)
- **bootstrap**: validate static settings targets (4f22ac144)
- **downloads**: preserve final targets on cancellation (3f024c4bb)
- **security**: make AppScope executable resource authority (5745d04e0)
- **settings**: close display language state races (aa31d4f4b)
- **settings**: centralize display language state (3caf6034d)

## 0.15.0 — 2026-09-07

### Breaking

- **Breaking** — delete the packaged lxapp icon and read every host's icon from the registry (d499bf5d3)
- **Breaking** — **lxapp**: address tab bar items by page name (1b011807e)

### Writing an lxapp

- **tabbar**: reopen iOS overflow after a folded-tab pick (1863c17a0)
- **tabbar**: dismiss overflow when the strip is rebuilt or torn down (35bdbe7de)
- **lxapp**: key cached artwork to its URL, and repaint expanded chrome (53ae1eda8)
- **lxapp**: look up one registry record per app, without a locale (0c5c97ae0)
- **lxapp**: report why a blocked open was refused, as data (3eec071d0)
- **lxapp**: re-fetch registry artwork the OS purged (70bf508c1)
- **lxapp**: keep the open gate off the icon download, and repaint on refresh (ac8bc96a5)
- **lxapp**: cache registry records and gate opening on a fresh status (fc8fc5211)
- **lxapp**: center the capsule on the navbar title (c72601b4c)
- **lxapp**: recover package loading without stale builtin overrides (1c950e623)
- **lxapp**: correct lifecycle test route and satisfy clippy (97799acb9)
- **lxapp**: preserve page lifecycle and navigation surfaces (74622fa51)
- **test**: stop claiming covers the recorder cannot emit (71fcb78c5)
- **test**: mark the eval capture envelope instead of sniffing its shape (4f9e7ea9e)
- **test**: count a capability as covered only when a spec reached it (2fbac4939)
- **logic**: require pull-down refresh enablement (f641c3814)
- **lxapp**: tell an lxapp whether it is on a mobile or a desktop machine (32f886bbb)
- **lxapp**: say what is wrong with a network URL in an icon path (fec976322)

### Embedding a host app

- **macos**: drop UIKit-only disable from overflow dismiss (50fffc4b6)
- **windows**: drop unused panel opener and a needless borrow (aa9ec6479)
- **windows**: resolve the About dialog's name and icon from the registry (719ab4ad0)
- **windows**: draw sidebar rows from the registry record (1f015b304)
- **android**: refresh open overflow panels with tabbar state (69b3dddb0)
- **harmony**: preserve hyphens in page show routes (ff34b6c94)
- **app**: let the home lxapp set the host display language (119f1b57e)
- **windows**: simulate the host class with the device frame (3865d2ceb)
- **android**: keep the pull-to-refresh indicator through an lxapp switch (c78153cfb)
- **android**: size the tab bar for the lxapp that is opening (6b28229bb)
- **macos**: stop the tab-bar "more" panel from flashing the window (aaaa1d54d)
- **macos**: slide a page only once it can draw itself (200e5e85e)
- **macos**: paint the page cover with the colour the page actually paints (acdf66420)
- **macos**: stop a page navigation reading as two animations (f4f08d243)
- **macos**: deliver shell sidebar actions to the shell, not the AppUI runtime (c8b6fc296)
- **macos**: give lxapp pages a real pull-to-refresh (3303962be)
- **device-io**: keep focus unconditional, guard only the call that asserts (7bd4eb84a)
- **device-io**: do not force main/key status on a window that refuses it (6cb31f6fb)

### Rust native extensions

- **control**: await initial page creation before open readiness (49a9820c4)
- **automation**: report which lx APIs a script actually reached (809c14679)

### CLI and CI

- **cli**: preserve provider configuration and platform build identity (0cd627083)
- **lxdev**: name the test run budget after its unit (abaac53c6)
- **runner**: make the simulated appearance reach the lxapp (4031ffe1d)
- **runner**: size an lxapp's capsule action icon like the built-in ones (29213f180)
- **cli**: keep the skill hand-off off the Windows build (607d4d6ea)
- **cli**: keep the agent skill and templates current on their own (c1d853241)
- **cli**: restore Apple package manifests after builds (327e910c6)
- **cli**: trust the platform's root certificates (cb51235a4)

### Docs and examples

- **showcase**: make the desktop cases hand the workspace back and stop guessing the host (1e4195e54)
- **showcase**: stop three new cases from assuming macOS behavior everywhere (88c5bc684)
- **showcase**: stop a scheduled redirect from leaking into the next case (89dd206f1)
- **types**: declare in_stack on LxAppRuntimeInfo (48a97df31)
- **showcase**: let the surface page receive opener messages (e2adebb82)

### Other

- **provider**: add a maintain state, and key cached icons on their URL (d1c33c7c1)
- **provider**: parse registry status case-insensitively (052c88c88)
- **provider**: add the lxapp registry contract (a7703de26)
- **chrome**: align tab overflow layouts and render local icons consistently (3932e1aba)
- give the tab bar overflow panel the colour it floats over (6d7ba7f4c)
- **upgrade**: fetch a pinned SDK the cache is still missing (42a1c6082)
- **upgrade**: name a retry that actually runs after a failed SDK fetch (3471fb8d6)

## 0.14.0 — 2026-09-01

### Breaking

- **Breaking** — **splash**: unify the fixed launch face and campaign handoff (78483d3a7)
- **Breaking** — **lxapp**: drop selectedIconPath; one icon per tab item (6cdfb363b)

### Writing an lxapp

- **splash**: close the launch-layer holes and drop the dark-face scaffolding (25aa83795)
- **theme**: let the host declare the page floor native chrome borders (89e06b191)
- **tabbar**: refine overflow navigation (3df40e429)
- **lxapp**: let a tab item declare the hosts it belongs on (5d462e0e7)
- **lxapp**: fold on the iOS phone strip, and mark "More" everywhere (4f4bcfc50)
- **lxapp**: draw the active indicator for every selected tab (29ad9726e)
- **lxapp**: make a single-icon tab read like a paired one when selected (75505dff3)
- **lxapp**: mark the active tab without a second icon (006956c36)
- **lxapp**: warm only the tab pages holding a strip slot (d4b56f068)
- **lxapp**: allow up to 10 tab items with a compact overflow split (50155df05)
- **lxapp**: expose window chrome selection (6291fe143)
- **lxapp**: load logic-disabled surface pages (0e57cd376)

### Embedding a host app

- **macos**: sync SwiftPM deployment target (b36306bb1)
- **ios**: compose the launch frame with a storyboard, not UILaunchScreen (d569d19cc)
- **android**: launch in the orientation the home page will use (09da37b3f)
- **android**: let a full-screen bottom surface reach the bottom edge (13eac1cad)
- **ios**: bundle packet tunnel extensions (7fba2ef3a)
- **config**: admit platform-specific lxapp roots (5cbdf757f)
- **windows**: key the overflow panel on declared indices (bacb9bc1f)
- **windows**: match tabbar overflow sheet (9415bfdd5)
- **macos**: make the tab overflow panel visible over the WebView (3158ee7a9)
- **windows**: fold extra tab items into a "More" slot (35b8ff85e)
- **harmony**: fold extra tab items into a "More" slot (38e016eb0)
- **apple**: fold extra tab items into a "More" slot (90096213c)
- **android**: fold extra tab items into a "More" slot (f87fa27f1)
- **macos**: restore full-chrome window behavior (b727a97ac)
- **macos**: clip tray panel corners cleanly (a9faa125c)
- **macos**: dismiss tray panel when opening window (8568b7d01)
- **windows**: realize shell before first frame (2eddcc5bb)
- **windows**: restore lxapp after browser closes (da34760ad)
- **macos**: keep capsule page chrome synchronized (d28f1afcc)
- **windows**: collapse nested if so clippy -D warnings passes (9338cf92e)
- **windows**: sync the runner capsule overlay on --capsule toggle (74016ec43)
- **platform**: present desktop file dialogs without parking the runloop (153bc940e)
- **android**: keep a tab page's WebView in the window between visits (0f8487cc1)
- **android**: actually run the launch cover's deferred restores (286273803)
- **windows**: make the runner device picker actually resize the frame (2c9300269)
- **harmony**: point the video settings button at an icon that exists (1fe06e783)

### Rust native extensions

- **lingxia**: keep page target out of root facade (0061709a7)
- **control**: let hosts extend the product CLI (74c697bf0)

### CLI and CI

- **ci**: stabilize main test suite (297a4a34e)
- **ci**: exclude the private provider checkout from the workspace (db442c9fa)
- **ci**: keep cloud checkout inside workspace (0550dd859)
- **runner**: fold tab items only in the phone shapes (2bbf51b21)
- **cli**: skip page action audit without logic (8fdf17e8f)
- **cli**: close upgrade review gaps (f8d8d38d2)
- **cli**: gate Windows upgrade rerun helper (7898ddc42)
- **cli**: close project upgrade safety gaps (5864c2be8)
- **cli**: harden project upgrade boundaries (611958fdc)
- **cli**: always upgrade CLI and prompt for project SDK line (b09a6a926)
- **cli**: project upgrade, version-train guard, sdk drift checks (c0be6294d)
- **cli**: harden credential readiness and rotation (d1dc6fb66)
- **cli**: separate LingXia auth from publish actions (d15fc86ce)
- **cli**: include all wallet entries in JSON status (2a80ff95d)
- **cli**: wallet-backed store credentials, artifact identity precheck, publish tokens per server (2f95b9bf8)
- **cli**: store Harmony AGC credentials per identity (e5d5fc1f5)
- **cli**: identity wallet, project binding, unified auth surface (cd311abcf)
- **cli**: route per-user state through lingxia_dir() (3fb8aa32b)
- **runner**: look up the built app by its actual bundle name (841998c02)
- **runner**: report the simulated capsule's geometry and make it optional (096f877f3)
- **cli**: catch a page entry that forwards only some of its actions (13249a68a)
- **cli**: add `lingxia browser-shell eject` (1c408a141)
- **release**: stop publishing the browser shell webui to npm (8b8005d71)
- **cli**: find the installed Runner bundle by extension, not by name (a39ccabf6)

### Docs and examples

- **showcase**: demo responsive tab bar overflow (e9130c988)
- **showcase**: keep the four declared tab items (c161e7f9d)

## 0.13.0 — 2026-08-26

### Writing an lxapp

- **update**: spell the developer channel `developer` everywhere (a9f06b5b8)
- **update**: install and update lxapps on the host build's channel (8d24d5f73)
- **upload**: keep the writer's own diagnosis when it is the cause (018c6a786)
- **upload**: report a denied host as denied, not as a dropped connection (1cb71e85f)
- **test**: add numeric ordering matchers, and report both numbers (bf5d50e72)
- **upload**: let lx.uploadFile send a raw body with PUT (5780ae992)

### Embedding a host app

- **android**: keep the launch canvas on the cover's colour until it is gone (e80f0b67a)
- **android**: let the launch cover own the system bars until it lifts (74e9e2b3e)
- **android**: reserve the bottom inset only while the TabBar is on screen (be7155fdf)
- **windows**: drop unused ReleaseType import (40f7fb11a)
- **media**: stop the capture before announcing that it stopped (#284) (d4cfc211c)

### CLI and CI

- **release**: find the Runner artifacts the CLI actually wrote (3cab7c928)
- **cli**: name artifacts from project name (b3d7170b3)
- **ci**: recycle flaky macOS automation once (e04326a01)
- **ci**: let cancelled PR automation exit promptly (4717559ec)
- **ci**: retry showcase dependency installs (484516304)
- **ci**: keep Windows cache uploads from failing PRs (a80c698af)
- **cli**: keep the Android 12 splash icon when no cover is configured (#285) (fce0466d1)
- **cli**: cover both Swift target layouts in the Apple ignore rules (cea8cd33b)
- **cli**: declare the lxapp icon the scaffold already writes (8d18e498f)
- **cli**: harden SDK dependency detection in Package.swift (9f5444892)
- **cli**: stop Apple build output landing in git status (ccf8c1b37)
- **cli**: inject the Apple SDK dependency for macOS builds too (2540f1901)
- **cli**: import Lingxia from com.lingxia.app in the Android template (9dc5059b2)
- **release**: let a CLI-only publish leave the workspace behind (a27590cc2)
- **cli**: stop asking to install a skill that is already there (cf32976f1)
- **cli**: name an embedded lxapp after itself, not only its host (7db55100a)
- **cli**: add lingxia upgrade (869332554)
- **cli**: derive the reported Rong version from the workspace (c9987418f)
- **release**: restore the CLI-only version bump (48eefa012)
- **cli**: route the docs at the built-in skill installer (aa94d5bfa)
- **cli**: address the review of the embedded skill (6fbdbfc41)
- **cli**: ship the agent skill inside the binary (b80ddb70c)
- **release**: install the packages workspace before building a member (0427f535c)

### Docs and examples

- **docs**: name @lingxia/test as the test SDK, not @rongjs/test (2032ca501)

## 0.12.0 — 2026-08-19

The first release this file covers. What came before it is the git log, not a
change list: entries begin here, and every release after this one records its
own changes in full.

0.12.0 is the whole of LingXia at one version — the runtime for standalone
lxapps and native host apps on Android, iOS, macOS, HarmonyOS, and Windows, the
`lingxia` and `lxdev` CLIs, the Android, Apple, and HarmonyOS SDKs, 29 crates on
crates.io, and 12 `@lingxia/*` npm packages. One of those is new: `@lingxia/test`,
an authoring SDK for lxapp tests.

LingXia is `0.x` and makes no compatibility promise. 0.12.0 breaks callers of
0.11 in places, and ships no migration guide — pin the version you build
against.
