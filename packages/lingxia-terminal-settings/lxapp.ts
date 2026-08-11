// The app entry an lxapp with `logic: true` is expected to have. Without it
// the Logic runtime loads the bundle but never finishes bootstrapping, so the
// settings page's handlers are never registered and its very first request —
// `loadTerminalSettings` — comes back as an unknown bridge method, leaving the
// screen on its unfilled defaults.
//
// Settings has nothing to do at launch: every action belongs to the page.
App({});
