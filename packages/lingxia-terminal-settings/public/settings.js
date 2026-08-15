(function () {
  "use strict";

  var snapshot = null;
  var draft = null;
  var themes = [];
  var hostThemeNames = new Set();
  var activeSlot = "dark";
  var dirty = false;
  var previewing = false;
  var pendingExternal = null;
  var unsubscribeState = null;
  var toastTimer = null;
  var windowsInlineImages = null;
  var installingInlineImages = false;

  function byId(id) { return document.getElementById(id); }
  function clone(value) { return JSON.parse(JSON.stringify(value)); }
  function tr(key, variables) {
    return window.LingXiaI18n ? window.LingXiaI18n.t(key, variables) : key;
  }
  function actions() {
    var bridge = window.LingXiaBridge;
    if (!bridge || !bridge.raw || typeof bridge.raw.call !== "function") {
      throw new Error("Terminal Settings Logic bridge is not available");
    }
    var call = function (name, input, options) {
      return bridge.raw.call(name, input, options).then(function (result) {
        if (result && result.ok === true) return result.value;
        var details = result && result.error ? result.error : {};
        var error = new Error(details.message || "Terminal Settings action failed");
        if (details.code) error.code = details.code;
        throw error;
      });
    };
    return {
      loadTerminalSettings: function () { return call("loadTerminalSettings"); },
      updateTerminalSettings: function (input) { return call("updateTerminalSettings", input); },
      resetTerminalSettings: function (input) { return call("resetTerminalSettings", input); },
      importTerminalScheme: function (input) { return call("importTerminalScheme", input); },
      previewTerminalScheme: function (input) { return call("previewTerminalScheme", input); },
      clearTerminalPreview: function () { return call("clearTerminalPreview"); },
      setWindowsInlineImages: function (input) { return call("setWindowsInlineImages", input, { timeoutMs: 0 }); }
    };
  }
  function message(error) {
    if (!error) return "Unknown error";
    if (typeof error === "string") return error;
    return error.message || error.error || JSON.stringify(error);
  }
  function toast(text, isError) {
    var node = byId("toast");
    node.textContent = text;
    node.classList.toggle("error", Boolean(isError));
    node.classList.add("show");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(function () { node.classList.remove("show"); }, 3200);
  }
  function setState(text) {
    var node = byId("state");
    node.removeAttribute("data-i18n");
    node.textContent = text;
  }
  function setDirty(next) {
    dirty = next;
    byId("save").disabled = !dirty;
    byId("actions").classList.toggle("visible", dirty);
    document.body.classList.toggle("has-actions", dirty);
    setState(dirty ? tr("app.state.dirty") : tr("app.state.clean"));
  }

  function color(theme, key, fallback) { return theme.scheme[key] || fallback; }
  function sourceLabel(source) {
    return source === "builtIn" ? tr("appearance.builtIn") : source === "imported" ? tr("appearance.imported") : source;
  }
  function label(entry) {
    return entry.displayName || entry.scheme.name || entry.name.replace(/(^|-)(.)/g, function (_, lead, letter) { return (lead ? " " : "") + letter.toUpperCase(); });
  }
  function rgb(hex) {
    var value = String(hex || "").trim().replace(/^#/, "");
    if (value.length === 3 || value.length === 4) value = value.slice(0, 3).split("").map(function (part) { return part + part; }).join("");
    if (value.length === 8) value = value.slice(0, 6);
    if (!/^[0-9a-f]{6}$/i.test(value)) return null;
    return [0, 2, 4].map(function (offset) { return parseInt(value.slice(offset, offset + 2), 16) / 255; });
  }
  function themeTone(scheme) {
    var channels = rgb(scheme.background);
    if (!channels) return activeSlot;
    var luminance = channels.map(function (value) {
      return value <= 0.04045 ? value / 12.92 : Math.pow((value + 0.055) / 1.055, 2.4);
    });
    return 0.2126 * luminance[0] + 0.7152 * luminance[1] + 0.0722 * luminance[2] >= 0.45 ? "light" : "dark";
  }
  function slotForMode() {
    if (draft.theme.mode === "light" || draft.theme.mode === "dark") return draft.theme.mode;
    return snapshot.effective.systemAppearance;
  }
  function themesForSlot(slot) {
    return themes.filter(function (entry) { return themeTone(entry.scheme) === slot; });
  }
  function ensureSelectionForSlot(slot) {
    var selected = themes.find(function (entry) { return entry.name === draft.theme[slot]; });
    if (selected && themeTone(selected.scheme) === slot) return false;
    var fallbackName = slot === "light" ? "lingxia-light" : "lingxia-dark";
    var fallback = themesForSlot(slot).find(function (entry) { return entry.name === fallbackName; }) || themesForSlot(slot)[0];
    if (!fallback) return false;
    draft.theme[slot] = fallback.name;
    return true;
  }
  function previewSelection() {
    var selected = themes.find(function (entry) { return entry.name === draft.theme[activeSlot]; });
    if (selected) beginPreview(selected.scheme);
  }
  function renderThemes() {
    var root = byId("themes");
    root.replaceChildren();
    themesForSlot(activeSlot).forEach(function (entry) {
      var scheme = entry.scheme;
      var card = document.createElement("button");
      card.type = "button";
      card.className = "theme" + (draft.theme[activeSlot] === entry.name ? " active" : "");
      card.setAttribute("aria-pressed", draft.theme[activeSlot] === entry.name ? "true" : "false");
      card.style.background = scheme.background;
      card.style.color = scheme.foreground;
      card.dataset.theme = entry.name;
      card.dataset.background = scheme.background;
      card.dataset.cursor = scheme.cursorColor || scheme.foreground;

      var name = document.createElement("span");
      name.className = "theme-name";
      name.textContent = label(entry);
      var source = document.createElement("span");
      source.className = "theme-source";
      source.textContent = entry.packaged ? entry.author : sourceLabel(entry.source);
      var sample = document.createElement("span");
      sample.className = "theme-sample";
      sample.textContent = "Aa 01 λ";
      var ramp = document.createElement("span");
      ramp.className = "ramp";
      ["red", "yellow", "green", "cyan", "blue", "purple"].forEach(function (key) {
        var dot = document.createElement("i"); dot.style.background = color(entry, key, scheme.foreground); ramp.appendChild(dot);
      });
      card.append(name, source, sample, ramp);

      if (entry.packaged) {
        var license = document.createElement("span");
        license.className = "theme-license";
        license.textContent = entry.spdx;
        license.title = entry.author + " · " + entry.upstream;
        card.appendChild(license);
      }

      card.addEventListener("click", function () {
        draft.theme[activeSlot] = entry.name;
        setDirty(true);
        renderThemes();
        beginPreview(entry.scheme);
      });
      root.appendChild(card);
    });
  }

  function beginPreview(scheme) {
    previewing = true;
    actions().previewTerminalScheme({ scheme: scheme }).catch(function (error) {
      previewing = false;
      toast("Preview failed: " + message(error), true);
    });
  }
  function endPreview() {
    if (!previewing) return;
    previewing = false;
    actions().clearTerminalPreview().catch(function () {});
  }

  function fill(snapshotValue) {
    previewing = false;
    snapshot = snapshotValue;
    pendingExternal = null;
    draft = clone(snapshot.value);
    activeSlot = slotForMode();
    var repairedSelection = ensureSelectionForSlot(activeSlot);
    if (window.LingXiaI18n) window.LingXiaI18n.apply();
    document.querySelectorAll("[data-mode]").forEach(function (button) {
      button.classList.toggle("active", button.dataset.mode === draft.theme.mode);
      button.setAttribute("aria-pressed", button.dataset.mode === draft.theme.mode ? "true" : "false");
    });
    byId("font-family").value = snapshot.effective.font.family;
    // The styled picker keeps the native select as its source of truth, but a
    // property assignment does not emit an event. Notify it after every fill
    // so an external settings change cannot leave the visible label stale.
    byId("font-family").dispatchEvent(new Event("change", { bubbles: true }));
    byId("font-size").value = Math.round(draft.font.size * 100) / 100;
    byId("line-height").value = Math.round(draft.font.lineHeight * 100) / 100;
    byId("ligatures").checked = draft.font.ligatures;
    renderThemes();
    renderWarnings();
    setDirty(repairedSelection);
  }

  function renderWarnings() {
    var root = byId("warnings");
    var messages = snapshot ? snapshot.warnings.map(function (warning) { return warning.message; }) : [];
    if (pendingExternal) messages.push(tr("app.externalChange"));
    root.replaceChildren();
    messages.forEach(function (text) {
      var item = document.createElement("p");
      item.textContent = text;
      root.appendChild(item);
    });
    root.hidden = messages.length === 0;
  }

  function collect() {
    var family = byId("font-family").value;
    if (family) draft.font.family = [family];
    draft.font.size = Number(byId("font-size").value);
    draft.font.lineHeight = Number(byId("line-height").value);
    draft.font.ligatures = byId("ligatures").checked;
    return draft;
  }

  function sameSettings(left, right) {
    return JSON.stringify(left) === JSON.stringify(right);
  }

  function acceptLoaded(result) {
    byId("windows-group").hidden = !result.isWindows;
    windowsInlineImages = result.windowsInlineImages;
    renderWindowsInlineImages();
    var hostThemes = result.themes;
    hostThemeNames = new Set(hostThemes.map(function (theme) { return theme.name; }));
    var byName = new Map();
    (window.LingXiaTerminalThemes || []).forEach(function (theme) { byName.set(theme.name, Object.assign({ packaged: true, source: "packaged" }, theme)); });
    hostThemes.forEach(function (theme) {
      var packaged = byName.get(theme.name);
      byName.set(theme.name, packaged ? Object.assign({}, packaged, theme, { packaged: true }) : theme);
    });
    themes = Array.from(byName.values());
    var select = byId("font-family");
    select.replaceChildren();
    var families = result.fonts.filter(function (font) { return font.monospace; }).map(function (font) { return font.family; });
    if (!families.includes(result.snapshot.effective.font.family)) families.push(result.snapshot.effective.font.family);
    families.sort(function (a, b) { return a.localeCompare(b); }).forEach(function (family) {
      var option = document.createElement("option"); option.value = family; option.textContent = family; select.appendChild(option);
    });
    fill(result.snapshot);
  }

  function formatBytes(bytes) {
    if (!Number.isFinite(bytes)) return "";
    return (bytes / (1024 * 1024)).toFixed(bytes >= 1024 * 1024 ? 1 : 2) + " MB";
  }

  function renderWindowsInlineImages(progress) {
    var toggle = byId("windows-inline-images");
    var progressRoot = byId("inline-image-progress");
    var bar = byId("inline-image-progress-bar");
    var label = byId("inline-image-progress-label");
    if (!toggle || byId("windows-group").hidden) return;
    toggle.checked = Boolean(windowsInlineImages && windowsInlineImages.enabled);
    toggle.disabled = installingInlineImages;
    progressRoot.hidden = !installingInlineImages;
    if (!installingInlineImages) return;
    var ratio = progress && Number.isFinite(progress.progress) ? progress.progress : null;
    bar.style.width = ratio === null ? "32%" : Math.max(2, Math.min(100, ratio * 100)) + "%";
    progressRoot.classList.toggle("indeterminate", ratio === null);
    var received = progress && formatBytes(progress.downloadedBytes);
    var total = progress && formatBytes(progress.totalBytes);
    label.textContent = received && total
      ? tr("windows.downloadingAmount", { received: received, total: total })
      : tr("windows.downloading");
  }

  function setWindowsInlineImages(enabled) {
    if (installingInlineImages) return;
    installingInlineImages = true;
    renderWindowsInlineImages();
    actions().setWindowsInlineImages({ enabled: enabled }).then(function (status) {
      windowsInlineImages = status;
      toast(enabled ? tr("windows.enabled") : tr("windows.disabled"));
    }).catch(function (error) {
      toast(tr("windows.failed", { message: message(error) }), true);
    }).finally(function () {
      installingInlineImages = false;
      renderWindowsInlineImages();
    });
  }

  function ensurePackaged(name) {
    var entry = themes.find(function (candidate) { return candidate.name === name && candidate.packaged; });
    if (!entry || hostThemeNames.has(name)) return Promise.resolve();
    return actions().importTerminalScheme({ text: JSON.stringify(entry.scheme), name: name }).then(function () { hostThemeNames.add(name); });
  }

  function save() {
    var next = clone(collect());
    byId("save").disabled = true;
    setState(tr("app.state.applying"));
    Promise.all(Array.from(new Set([next.theme.light, next.theme.dark])).map(ensurePackaged))
      .then(function () { return actions().updateTerminalSettings({ patch: next, ifRevision: snapshot.revision }); })
      .then(function (value) { fill(value); toast(tr("app.applied")); })
      .catch(function (error) {
        if (error && error.code === "E_TERMINAL_REVISION_CONFLICT") {
          toast(tr("app.conflict"), true);
          load();
          return;
        }
        // A bridge timeout is ambiguous: the atomic write may already have
        // committed. Reconcile before telling the user that Apply failed.
        actions().loadTerminalSettings().then(function (result) {
          if (sameSettings(result.snapshot.value, next)) {
            acceptLoaded(result);
            toast(tr("app.applied"));
            return;
          }
          byId("save").disabled = false;
          setState("Could not apply");
          toast(message(error), true);
        }).catch(function () {
          byId("save").disabled = false;
          setState("Could not apply");
          toast(message(error), true);
        });
      });
  }

  function reset(scope) {
    endPreview();
    byId("actions").classList.add("visible");
    document.body.classList.add("has-actions");
    setState(tr("app.state.resetting"));
    actions().resetTerminalSettings(scope === "all"
      ? { ifRevision: snapshot.revision }
      : { scope: scope, ifRevision: snapshot.revision })
      .then(function (value) { fill(value); toast(scope === "all" ? tr("reset.done") : (scope === "font" ? tr("reset.typeDone") : tr("reset.appearanceDone"))); })
      .catch(function (error) {
        if (error && error.code === "E_TERMINAL_REVISION_CONFLICT") {
          toast(tr("app.conflict"), true);
          load();
          return;
        }
        setState("Could not reset");
        toast(message(error), true);
      });
  }

  function load() {
    actions().loadTerminalSettings().then(function (result) {
      acceptLoaded(result);
    }).catch(function (error) {
      setState("Settings unavailable");
      toast("This settings package is not compatible with the host: " + message(error), true);
    });
  }

  function acceptExternalSnapshot(next) {
    if (!next || !snapshot || JSON.stringify(next) === JSON.stringify(snapshot)) return;
    if (dirty && next.revision !== snapshot.revision) {
      pendingExternal = next;
      renderWarnings();
      toast(tr("app.externalChange"), true);
      return;
    }
    if (dirty) {
      var previousSlot = activeSlot;
      snapshot = next;
      activeSlot = slotForMode();
      ensureSelectionForSlot(activeSlot);
      renderThemes();
      renderWarnings();
      if (previewing && activeSlot !== previousSlot) previewSelection();
      return;
    }
    fill(next);
  }

  function subscribeToLogicState() {
    var bridge = window.LingXiaBridge;
    if (!bridge || !bridge.state || typeof bridge.state.subscribe !== "function") return;
    unsubscribeState = bridge.state.subscribe(function (state) {
      acceptExternalSnapshot(state && state.terminalSettingsSnapshot);
      if (installingInlineImages) renderWindowsInlineImages(state && state.windowsInlineImageProgress);
    });
  }

  document.querySelectorAll("[data-mode]").forEach(function (button) {
    button.addEventListener("click", function () {
      draft.theme.mode = button.dataset.mode;
      document.querySelectorAll("[data-mode]").forEach(function (item) {
        item.classList.toggle("active", item === button);
        item.setAttribute("aria-pressed", item === button ? "true" : "false");
      });
      activeSlot = slotForMode();
      ensureSelectionForSlot(activeSlot);
      setDirty(true);
      renderThemes();
      previewSelection();
    });
  });
  document.querySelectorAll("[data-reset]").forEach(function (button) { button.addEventListener("click", function () { reset(button.dataset.reset); }); });
  ["font-family", "font-size", "line-height", "ligatures"].forEach(function (id) {
    byId(id).addEventListener("input", function () { collect(); setDirty(true); });
  });
  byId("theme-file").addEventListener("change", function () {
    var file = this.files && this.files[0];
    if (!file) return;
    file.text().then(function (text) { return actions().importTerminalScheme({ text: text, name: file.name.replace(/\.[^.]+$/, "") }); })
      .then(function (result) { toast("Imported " + result.name); return load(); })
      .catch(function (error) { toast("Could not import theme: " + message(error), true); });
    this.value = "";
  });
  var language = byId("language");
  if (language && window.LingXiaI18n) {
    // No local override means this screen follows the app's language, which is
    // what the host reports — not the operating system's. Labelling that
    // "system" claimed otherwise while the screen rendered in the app's locale.
    language.value = localStorage.getItem(window.LingXiaI18n.storageKey) || "app";
    language.addEventListener("change", function () {
      // Screen-local by design; terminal appearance config does not own the
      // locale of this settings package.
      if (language.value === "app") window.LingXiaI18n.followApp();
      else window.LingXiaI18n.setLocale(language.value);
      window.LingXiaI18n.apply();
      if (snapshot) {
        renderWarnings();
        renderThemes();
        setState(dirty ? tr("app.state.dirty") : tr("app.state.clean"));
      }
    });
  }

  byId("save").addEventListener("click", save);
  byId("windows-inline-images").addEventListener("change", function () {
    setWindowsInlineImages(this.checked);
  });
  window.addEventListener("pagehide", function () {
    endPreview();
    if (unsubscribeState) unsubscribeState();
  });
  subscribeToLogicState();
  load();
})();
