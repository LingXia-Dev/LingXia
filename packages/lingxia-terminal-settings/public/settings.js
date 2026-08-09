(function () {
  "use strict";

  var snapshot = null;
  var draft = null;
  var themes = [];
  var hostThemeNames = new Set();
  var activeSlot = "dark";
  var dirty = false;
  var previewing = false;
  var toastTimer = null;

  function byId(id) { return document.getElementById(id); }
  function clone(value) { return JSON.parse(JSON.stringify(value)); }
  function bridge() {
    var value = window.LingXiaBridge;
    if (!value || typeof value.invoke !== "function") throw new Error("LingXiaBridge is not available");
    return value;
  }
  function invoke(route, input) { return bridge().invoke(route, input); }
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
  function setState(text) { byId("state").textContent = text; }
  function setDirty(next) {
    dirty = next;
    byId("save").disabled = !dirty;
    setState(dirty ? "Unsaved changes" : "Up to date");
  }

  function color(theme, key, fallback) { return theme.scheme[key] || fallback; }
  function sourceLabel(source) {
    return source === "builtIn" ? "Built in" : source === "imported" ? "Imported" : source;
  }
  function label(entry) {
    return entry.displayName || entry.scheme.name || entry.name.replace(/(^|-)(.)/g, function (_, lead, letter) { return (lead ? " " : "") + letter.toUpperCase(); });
  }
  function renderThemes() {
    var root = byId("themes");
    root.replaceChildren();
    themes.forEach(function (entry) {
      var scheme = entry.scheme;
      var card = document.createElement("button");
      card.type = "button";
      card.className = "theme" + (draft.theme[activeSlot] === entry.name ? " active" : "");
      card.style.background = scheme.background;
      card.style.color = scheme.foreground;
      card.dataset.theme = entry.name;

      var name = document.createElement("span");
      name.className = "theme-name";
      name.textContent = label(entry);
      var source = document.createElement("span");
      source.className = "theme-source";
      source.textContent = entry.packaged ? entry.author + " · " + entry.spdx : sourceLabel(entry.source);
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
      });
      card.addEventListener("pointerenter", function () { beginPreview(entry.scheme); });
      card.addEventListener("pointerleave", endPreview);
      card.addEventListener("focus", function () { beginPreview(entry.scheme); });
      card.addEventListener("blur", endPreview);
      root.appendChild(card);
    });
  }

  function beginPreview(scheme) {
    previewing = true;
    invoke("terminal.themes.preview", { scheme: scheme }).catch(function (error) {
      previewing = false;
      toast("Preview failed: " + message(error), true);
    });
  }
  function endPreview() {
    if (!previewing) return;
    previewing = false;
    invoke("terminal.themes.previewEnd").catch(function () {});
  }

  function fill(snapshotValue) {
    snapshot = snapshotValue;
    draft = clone(snapshot.config);
    document.querySelectorAll("[data-mode]").forEach(function (button) {
      button.classList.toggle("active", button.dataset.mode === draft.theme.mode);
    });
    byId("families").value = draft.font.family.join(", ");
    byId("font-size").value = draft.font.size;
    byId("line-height").value = draft.font.lineHeight;
    byId("bold").value = draft.font.bold;
    byId("ligatures").checked = draft.font.ligatures;
    byId("opacity").value = draft.theme.opacity;
    byId("opacity-value").value = Math.round(draft.theme.opacity * 100) + "%";
    byId("cursor-style").value = draft.theme.cursor.style;
    byId("cursor-blink").checked = draft.theme.cursor.blink;
    byId("config-path").textContent = snapshot.path;
    var resolved = snapshot.resolvedFont;
    byId("resolved-font").textContent = resolved.fellBack
      ? "Using system fallback; missing " + resolved.missing.join(", ")
      : "Using " + resolved.family + (resolved.missing.length ? " after " + resolved.missing.join(", ") : "");
    renderThemes();
    setDirty(false);
  }

  function collect() {
    var families = byId("families").value.split(",").map(function (value) { return value.trim(); }).filter(Boolean);
    draft.font.family = families;
    draft.font.size = Number(byId("font-size").value);
    draft.font.lineHeight = Number(byId("line-height").value);
    draft.font.bold = byId("bold").value;
    draft.font.ligatures = byId("ligatures").checked;
    draft.theme.opacity = Number(byId("opacity").value);
    draft.theme.cursor.style = byId("cursor-style").value;
    draft.theme.cursor.blink = byId("cursor-blink").checked;
    return draft;
  }

  function ensurePackaged(name) {
    var entry = themes.find(function (candidate) { return candidate.name === name && candidate.packaged; });
    if (!entry || hostThemeNames.has(name)) return Promise.resolve();
    return invoke("terminal.themes.import", { text: JSON.stringify(entry.scheme), name: name }).then(function () { hostThemeNames.add(name); });
  }

  function save() {
    var next = collect();
    byId("save").disabled = true;
    setState("Applying…");
    Promise.all([ensurePackaged(next.theme.light), ensurePackaged(next.theme.dark)])
      .then(function () { return invoke("terminal.config.apply", next); })
      .then(function (value) { fill(value); toast("Applied to every open terminal"); })
      .catch(function (error) { byId("save").disabled = false; setState("Could not apply"); toast(message(error), true); });
  }

  function reset(scope) {
    endPreview();
    setState("Resetting…");
    invoke("terminal.config.reset", scope === "all" ? {} : { scope: scope })
      .then(function (value) { fill(value); toast(scope === "all" ? "Terminal settings reset" : (scope === "font" ? "Type reset" : "Appearance reset")); })
      .catch(function (error) { setState("Could not reset"); toast(message(error), true); });
  }

  function load() {
    Promise.all([
      invoke("terminal.config.get"),
      invoke("terminal.themes.list"),
      invoke("terminal.fonts.list")
    ]).then(function (values) {
      var hostThemes = values[1];
      hostThemeNames = new Set(hostThemes.map(function (theme) { return theme.name; }));
      var byName = new Map();
      (window.LingXiaTerminalThemes || []).forEach(function (theme) { byName.set(theme.name, Object.assign({ packaged: true, source: "packaged" }, theme)); });
      hostThemes.forEach(function (theme) {
        var packaged = byName.get(theme.name);
        byName.set(theme.name, packaged ? Object.assign({}, packaged, theme, { packaged: true }) : theme);
      });
      themes = Array.from(byName.values());
      var select = byId("installed-fonts");
      select.replaceChildren(new Option("Add a family…", ""));
      values[2].filter(function (font) { return font.monospace; }).sort(function (a, b) { return a.family.localeCompare(b.family); }).forEach(function (font) {
        var option = document.createElement("option"); option.value = font.family; option.textContent = font.family + (font.nerdIcons ? "  ◆" : "") + (font.ligatures ? "  ƒ" : ""); select.appendChild(option);
      });
      fill(values[0]);
    }).catch(function (error) {
      setState("Settings unavailable");
      toast("This settings package is not compatible with the host: " + message(error), true);
    });
  }

  document.querySelectorAll("[data-mode]").forEach(function (button) {
    button.addEventListener("click", function () {
      draft.theme.mode = button.dataset.mode;
      document.querySelectorAll("[data-mode]").forEach(function (item) { item.classList.toggle("active", item === button); });
      setDirty(true);
    });
  });
  document.querySelectorAll("[data-slot]").forEach(function (button) {
    button.addEventListener("click", function () {
      activeSlot = button.dataset.slot;
      document.querySelectorAll("[data-slot]").forEach(function (item) { item.classList.toggle("active", item === button); });
      renderThemes();
    });
  });
  document.querySelectorAll("[data-reset]").forEach(function (button) { button.addEventListener("click", function () { reset(button.dataset.reset); }); });
  ["families", "font-size", "line-height", "bold", "ligatures", "opacity", "cursor-style", "cursor-blink"].forEach(function (id) {
    byId(id).addEventListener("input", function () { collect(); if (id === "opacity") byId("opacity-value").value = Math.round(Number(byId("opacity").value) * 100) + "%"; setDirty(true); });
  });
  byId("installed-fonts").addEventListener("change", function () {
    if (!this.value) return;
    var current = byId("families").value.split(",").map(function (value) { return value.trim(); }).filter(Boolean);
    byId("families").value = [this.value].concat(current.filter(function (value) { return value.toLowerCase() !== this.value.toLowerCase(); }, this)).join(", ");
    this.value = ""; collect(); setDirty(true);
  });
  byId("theme-file").addEventListener("change", function () {
    var file = this.files && this.files[0];
    if (!file) return;
    file.text().then(function (text) { return invoke("terminal.themes.import", { text: text, name: file.name.replace(/\.[^.]+$/, "") }); })
      .then(function (result) { toast("Imported " + result.name); return load(); })
      .catch(function (error) { toast("Could not import theme: " + message(error), true); });
    this.value = "";
  });
  byId("save").addEventListener("click", save);
  window.addEventListener("pagehide", endPreview);
  load();
})();
