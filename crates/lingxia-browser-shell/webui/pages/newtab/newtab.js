(function () {
  'use strict';

  var i18n = window.LingXiaI18n;
  var t = i18n.t;
  var SETTINGS_KEY = 'lingxia.newtab.settings.v1';
  var DB_NAME = 'lingxia-newtab';
  var DB_VERSION = 1;
  var STORE_NAME = 'assets';
  var BACKGROUND_KEY = 'background';
  var MAX_IMAGE_BYTES = 25 * 1024 * 1024;
  var MAX_SHORTCUTS = 12;
  var BUILTIN_BING = {
    id: 'bing',
    name: 'Bing',
    url: 'https://www.bing.com/search?q={query}',
    builtin: true
  };
  var BUILTIN_GOOGLE = {
    id: 'google',
    name: 'Google',
    url: 'https://www.google.com/search?q={query}',
    builtin: true
  };
  var BUILTIN_ENGINES = [BUILTIN_BING, BUILTIN_GOOGLE];

  var state = loadSettings();
  var activeBackgroundUrl = null;
  var toastTimer = null;

  function loadSettings() {
    var fallback = { defaultEngineId: BUILTIN_BING.id, engines: BUILTIN_ENGINES.slice(), shortcuts: [] };
    try {
      var parsed = JSON.parse(localStorage.getItem(SETTINGS_KEY) || 'null');
      if (!parsed || !Array.isArray(parsed.engines)) return fallback;
      var shortcuts = (Array.isArray(parsed.shortcuts) ? parsed.shortcuts : []).filter(validShortcut)
        .slice(0, MAX_SHORTCUTS)
        .map(function (shortcut) {
          return { id: shortcut.id, name: shortcut.name, url: shortcut.url };
        });
      var custom = parsed.engines.filter(validStoredEngine).filter(function (engine) {
        return !BUILTIN_ENGINES.some(function (builtin) {
          return engine.id === builtin.id || engine.url.toLowerCase() === builtin.url.toLowerCase();
        });
      }).map(function (engine) {
        return { id: engine.id, name: engine.name, url: engine.url, builtin: false };
      });
      var engines = BUILTIN_ENGINES.concat(custom);
      var defaultEngineId = engines.some(function (engine) { return engine.id === parsed.defaultEngineId; })
        ? parsed.defaultEngineId
        : BUILTIN_BING.id;
      return { defaultEngineId: defaultEngineId, engines: engines, shortcuts: shortcuts };
    } catch (_) {
      return fallback;
    }
  }

  function validShortcut(shortcut) {
    return shortcut && typeof shortcut.id === 'string' && typeof shortcut.name === 'string' &&
      typeof shortcut.url === 'string' && validShortcutUrl(shortcut.url);
  }

  function validShortcutUrl(value) {
    try {
      var parsed = new URL(value);
      return parsed.protocol === 'http:' || parsed.protocol === 'https:';
    } catch (_) {
      return false;
    }
  }

  function validStoredEngine(engine) {
    return engine && typeof engine.id === 'string' && typeof engine.name === 'string' &&
      typeof engine.url === 'string' && validSearchUrl(engine.url);
  }

  function validSearchUrl(value) {
    if (typeof value !== 'string' || !value.includes('{query}')) return false;
    try {
      var parsed = new URL(value.replace('{query}', 'test'));
      return parsed.protocol === 'http:' || parsed.protocol === 'https:';
    } catch (_) {
      return false;
    }
  }

  function saveSettings(showConfirmation) {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({
      defaultEngineId: state.defaultEngineId,
      engines: state.engines.filter(function (engine) { return !engine.builtin; }).map(function (engine) {
        return { id: engine.id, name: engine.name, url: engine.url };
      }),
      shortcuts: state.shortcuts
    }));
    renderEngines();
    syncActiveEngine();
    renderShortcuts();
    if (showConfirmation) toast(t('newtab.settingsSaved'));
  }

  function activeEngine() {
    return state.engines.find(function (engine) { return engine.id === state.defaultEngineId; }) || BUILTIN_BING;
  }

  function syncActiveEngine() {
    var engine = activeEngine();
    document.getElementById('engineMark').textContent = engine.name.trim().charAt(0).toUpperCase() || 'S';
  }

  function renderEngines() {
    var list = document.getElementById('engineList');
    list.replaceChildren();
    state.engines.forEach(function (engine) {
      var row = document.createElement('label');
      row.className = 'engine-row';

      var radio = document.createElement('input');
      radio.type = 'radio';
      radio.name = 'defaultEngine';
      radio.value = engine.id;
      radio.checked = engine.id === state.defaultEngineId;
      radio.setAttribute('aria-label', t('newtab.defaultEngine') + ': ' + engine.name);
      radio.addEventListener('change', function () {
        state.defaultEngineId = engine.id;
        saveSettings(true);
      });

      var copy = document.createElement('span');
      copy.className = 'engine-copy';
      var name = document.createElement('span');
      name.className = 'engine-name';
      name.textContent = engine.name;
      var url = document.createElement('span');
      url.className = 'engine-url';
      url.textContent = engine.url;
      copy.append(name, url);

      var action = document.createElement('span');
      if (!engine.builtin) {
        var remove = document.createElement('button');
        remove.className = 'icon-button';
        remove.type = 'button';
        remove.title = t('common.delete');
        remove.setAttribute('aria-label', t('common.delete') + ': ' + engine.name);
        remove.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4.5 7h15M9 7V4.5h6V7m-8.5 0 .75 13h9.5l.75-13M10 10.5v6M14 10.5v6"/></svg>';
        remove.addEventListener('click', function (event) {
          event.preventDefault();
          state.engines = state.engines.filter(function (candidate) { return candidate.id !== engine.id; });
          if (state.defaultEngineId === engine.id) state.defaultEngineId = BUILTIN_BING.id;
          saveSettings(true);
        });
        action.appendChild(remove);
      }

      row.append(radio, copy, action);
      list.appendChild(row);
    });
  }

  function renderShortcuts() {
    var grid = document.getElementById('shortcutGrid');
    grid.replaceChildren();
    state.shortcuts.forEach(function (shortcut) {
      var tile = document.createElement('a');
      tile.className = 'shortcut';
      tile.href = shortcut.url;
      tile.title = shortcut.url;

      var mark = document.createElement('span');
      mark.className = 'shortcut-mark';
      mark.setAttribute('aria-hidden', 'true');
      var letter = shortcut.name.trim().charAt(0).toUpperCase() || '?';
      var origin = originOf(shortcut.url);
      if (origin) {
        var icon = document.createElement('img');
        icon.src = origin + '/favicon.ico';
        icon.alt = '';
        icon.loading = 'lazy';
        icon.addEventListener('error', function () { mark.textContent = letter; });
        mark.appendChild(icon);
      } else {
        mark.textContent = letter;
      }

      var label = document.createElement('span');
      label.className = 'shortcut-label';
      label.textContent = shortcut.name;

      var remove = document.createElement('button');
      remove.className = 'shortcut-remove';
      remove.type = 'button';
      remove.title = t('newtab.removeShortcut');
      remove.setAttribute('aria-label', t('newtab.removeShortcut') + ': ' + shortcut.name);
      remove.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18"/></svg>';
      remove.addEventListener('click', function (event) {
        event.preventDefault();
        event.stopPropagation();
        state.shortcuts = state.shortcuts.filter(function (candidate) { return candidate.id !== shortcut.id; });
        saveSettings(true);
      });

      tile.append(mark, label, remove);
      grid.appendChild(tile);
    });

    if (state.shortcuts.length < MAX_SHORTCUTS) {
      var add = document.createElement('button');
      add.className = 'shortcut shortcut-add';
      add.type = 'button';
      add.id = 'addShortcut';
      add.title = t('newtab.addShortcut');
      add.setAttribute('aria-label', t('newtab.addShortcut'));
      var addMark = document.createElement('span');
      addMark.className = 'shortcut-mark';
      addMark.setAttribute('aria-hidden', 'true');
      addMark.textContent = '+';
      add.append(addMark);
      add.addEventListener('click', openShortcutDialog);
      grid.appendChild(add);
    }
  }

  function openShortcutDialog() {
    document.getElementById('shortcutDialog').classList.add('open');
    document.getElementById('shortcutName').focus();
  }

  function closeShortcutDialog() {
    document.getElementById('shortcutDialog').classList.remove('open');
    document.getElementById('shortcutForm').reset();
    document.getElementById('shortcutError').classList.remove('visible');
  }

  function normalizeShortcutUrl(value) {
    if (!value) return '';
    return /^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(value) ? value : 'https://' + value;
  }

  function originOf(url) {
    try {
      return new URL(url).origin;
    } catch (_) {
      return '';
    }
  }

  function loadBuildInfo() {
    var api = window.LingXiaBridge;
    if (!api || typeof api.invoke !== 'function') return;
    Promise.resolve(api.invoke('app.getInfo')).then(function (info) {
      if (!info) return;
      var parts = [];
      if (info.webuiVersion) parts.push('v' + info.webuiVersion);
      if (info.gitSha) parts.push(info.gitSha);
      document.getElementById('buildInfo').textContent = parts.join(' · ');
    }).catch(function () {});
  }

  function openDatabase() {
    return new Promise(function (resolve, reject) {
      var request = indexedDB.open(DB_NAME, DB_VERSION);
      request.onupgradeneeded = function () {
        if (!request.result.objectStoreNames.contains(STORE_NAME)) {
          request.result.createObjectStore(STORE_NAME);
        }
      };
      request.onsuccess = function () { resolve(request.result); };
      request.onerror = function () { reject(request.error); };
    });
  }

  async function backgroundStore(mode, value) {
    var db = await openDatabase();
    try {
      return await new Promise(function (resolve, reject) {
        var tx = db.transaction(STORE_NAME, mode === 'get' ? 'readonly' : 'readwrite');
        var store = tx.objectStore(STORE_NAME);
        var result;
        var request = mode === 'get'
          ? store.get(BACKGROUND_KEY)
          : mode === 'put'
            ? store.put(value, BACKGROUND_KEY)
            : store.delete(BACKGROUND_KEY);
        request.onsuccess = function () { result = request.result; };
        request.onerror = function () { reject(request.error); };
        tx.oncomplete = function () { resolve(result); };
        tx.onerror = function () { reject(tx.error); };
        tx.onabort = function () { reject(tx.error || new Error('background storage transaction aborted')); };
      });
    } finally {
      db.close();
    }
  }

  function applyBackground(blob) {
    if (activeBackgroundUrl) URL.revokeObjectURL(activeBackgroundUrl);
    activeBackgroundUrl = blob ? URL.createObjectURL(blob) : null;
    document.body.style.backgroundImage = activeBackgroundUrl ? 'url("' + activeBackgroundUrl + '")' : '';
    document.body.classList.toggle('has-background', Boolean(activeBackgroundUrl));
    var preview = document.getElementById('backgroundPreview');
    preview.style.backgroundImage = activeBackgroundUrl ? 'url("' + activeBackgroundUrl + '")' : '';
    preview.classList.toggle('has-image', Boolean(activeBackgroundUrl));
    document.getElementById('chooseBackground').textContent = t(
      activeBackgroundUrl ? 'newtab.replaceImage' : 'newtab.chooseImage'
    );
    document.getElementById('removeBackground').disabled = !activeBackgroundUrl;
  }

  async function loadBackground() {
    try {
      var record = await backgroundStore('get');
      applyBackground(record && record.blob instanceof Blob ? record.blob : null);
    } catch (_) {
      applyBackground(null);
    }
  }

  function normalizeImage(file) {
    return new Promise(function (resolve, reject) {
      var sourceUrl = URL.createObjectURL(file);
      var image = new Image();
      image.onload = function () {
        URL.revokeObjectURL(sourceUrl);
        var maxDimension = 2560;
        var scale = Math.min(1, maxDimension / Math.max(image.naturalWidth, image.naturalHeight));
        if (scale === 1 && file.size <= 8 * 1024 * 1024) {
          resolve(file);
          return;
        }
        var canvas = document.createElement('canvas');
        canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
        canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));
        var context = canvas.getContext('2d');
        context.drawImage(image, 0, 0, canvas.width, canvas.height);
        canvas.toBlob(function (blob) {
          blob ? resolve(blob) : reject(new Error('image encode failed'));
        }, 'image/jpeg', 0.9);
      };
      image.onerror = function () {
        URL.revokeObjectURL(sourceUrl);
        reject(new Error('image decode failed'));
      };
      image.src = sourceUrl;
    });
  }

  function toast(message) {
    var node = document.getElementById('toast');
    if (toastTimer) clearTimeout(toastTimer);
    node.textContent = message;
    node.classList.add('visible');
    toastTimer = setTimeout(function () { node.classList.remove('visible'); }, 2200);
  }

  function closeEngineForm() {
    document.getElementById('engineForm').classList.remove('open');
    document.getElementById('engineForm').reset();
    document.getElementById('engineError').classList.remove('visible');
  }

  i18n.apply();
  syncActiveEngine();
  renderEngines();
  renderShortcuts();
  loadBackground();
  loadBuildInfo();

  document.getElementById('searchForm').addEventListener('submit', function (event) {
    event.preventDefault();
    var query = document.getElementById('searchInput').value.trim();
    if (!query) return;
    location.href = activeEngine().url.split('{query}').join(encodeURIComponent(query));
  });

  var overlay = document.getElementById('customizeOverlay');
  document.getElementById('customizeButton').addEventListener('click', function () {
    overlay.classList.add('open');
    document.getElementById('closeCustomize').focus();
  });
  document.getElementById('closeCustomize').addEventListener('click', function () {
    overlay.classList.remove('open');
    closeEngineForm();
  });
  overlay.addEventListener('click', function (event) {
    if (event.target === overlay) {
      overlay.classList.remove('open');
      closeEngineForm();
    }
  });
  document.addEventListener('keydown', function (event) {
    if (event.key !== 'Escape') return;
    if (document.getElementById('shortcutDialog').classList.contains('open')) {
      closeShortcutDialog();
      return;
    }
    if (overlay.classList.contains('open')) {
      overlay.classList.remove('open');
      closeEngineForm();
      document.getElementById('customizeButton').focus();
    }
  });

  var shortcutDialog = document.getElementById('shortcutDialog');
  shortcutDialog.addEventListener('click', function (event) {
    if (event.target === shortcutDialog) closeShortcutDialog();
  });
  document.getElementById('cancelShortcut').addEventListener('click', closeShortcutDialog);
  document.getElementById('shortcutForm').addEventListener('submit', function (event) {
    event.preventDefault();
    var name = document.getElementById('shortcutName').value.trim();
    var url = normalizeShortcutUrl(document.getElementById('shortcutUrl').value.trim());
    var error = document.getElementById('shortcutError');
    if (!name || !validShortcutUrl(url)) {
      error.textContent = t('newtab.invalidShortcut');
      error.classList.add('visible');
      return;
    }
    var normalized = url.toLowerCase();
    if (state.shortcuts.some(function (shortcut) { return shortcut.url.toLowerCase() === normalized; })) {
      error.textContent = t('newtab.duplicateShortcut');
      error.classList.add('visible');
      return;
    }
    var id = 'shortcut-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 7);
    state.shortcuts.push({ id: id, name: name, url: url });
    closeShortcutDialog();
    saveSettings(true);
  });

  document.getElementById('showEngineForm').addEventListener('click', function () {
    document.getElementById('engineForm').classList.add('open');
    document.getElementById('engineName').focus();
  });
  document.getElementById('cancelEngine').addEventListener('click', closeEngineForm);
  document.getElementById('engineForm').addEventListener('submit', function (event) {
    event.preventDefault();
    var name = document.getElementById('engineName').value.trim();
    var url = document.getElementById('engineUrl').value.trim();
    var error = document.getElementById('engineError');
    if (!name || !validSearchUrl(url)) {
      error.textContent = t('newtab.invalidEngine');
      error.classList.add('visible');
      return;
    }
    var normalized = url.toLowerCase();
    if (state.engines.some(function (engine) { return engine.url.toLowerCase() === normalized; })) {
      error.textContent = t('newtab.duplicateEngine');
      error.classList.add('visible');
      return;
    }
    var id = 'custom-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 7);
    state.engines.push({ id: id, name: name, url: url, builtin: false });
    state.defaultEngineId = id;
    closeEngineForm();
    saveSettings(true);
  });

  var fileInput = document.getElementById('backgroundFile');
  document.getElementById('chooseBackground').addEventListener('click', function () { fileInput.click(); });
  fileInput.addEventListener('change', async function () {
    var file = fileInput.files && fileInput.files[0];
    fileInput.value = '';
    if (!file) return;
    if (!file.type.startsWith('image/') || file.size > MAX_IMAGE_BYTES) {
      toast(t('newtab.imageTooLarge'));
      return;
    }
    try {
      var blob = await normalizeImage(file);
      await backgroundStore('put', { blob: blob, updatedAt: Date.now() });
      applyBackground(blob);
      toast(t('newtab.settingsSaved'));
    } catch (_) {
      toast(t('newtab.imageReadFailed'));
    }
  });
  document.getElementById('removeBackground').addEventListener('click', async function () {
    try {
      await backgroundStore('delete');
      applyBackground(null);
      toast(t('newtab.settingsSaved'));
    } catch (_) {
      toast(t('newtab.imageReadFailed'));
    }
  });

  window.addEventListener('beforeunload', function () {
    if (activeBackgroundUrl) URL.revokeObjectURL(activeBackgroundUrl);
  });
})();
