(function (global) {
  'use strict';

  /*
   * Same contract as the browser package's i18n: `data-i18n` attributes in the
   * markup, a two-locale dictionary, and the host's language followed live
   * through `settings.watchLanguage`. Sharing the shape matters more than
   * sharing the file - a settings screen that picked its own convention would
   * be the one surface a translator has to learn twice.
   */
  var dictionaries = {
    'en-US': {
      'app.title': 'Terminal',
      'app.apply': 'Apply',
      'app.state.loading': 'Loading',
      'app.state.clean': 'No changes',
      'app.state.dirty': 'Unsaved changes',
      'app.state.unavailable': 'Settings unavailable',

      'general.title': 'General',
      'general.language': 'Language',
      'general.languageHint': 'Applies to this screen. Not saved yet.',
      'general.systemDefault': 'System default',

      'appearance.title': 'Appearance',
      'appearance.mode': 'Mode',
      'appearance.modeHint': 'Which scheme a terminal uses.',
      'appearance.system': 'System',
      'appearance.light': 'Light',
      'appearance.dark': 'Dark',
      'appearance.scheme': 'Color scheme',
      'appearance.schemeHint': 'Hover a scheme to preview it in every open terminal. Light and dark are chosen separately.',
      'appearance.empty': 'No schemes yet. Import one below.',
      'appearance.import': 'Import a scheme',
      'appearance.importHint': 'Windows Terminal JSON, Xresources, or kitty conf.',
      'appearance.choose': 'Choose file',

      'type.title': 'Type',
      'type.families': 'Font candidates',
      'type.familiesHint': 'The first family installed on this machine wins.',
      'type.installed': 'Installed families',
      'type.installedHint': 'Append one to the list above.',
      'type.add': 'Add a family',
      'type.size': 'Size',
      'type.lineHeight': 'Line height',
      'type.lineHeightHint': 'A multiple of the font size.',
      'type.bold': 'Bold text',
      'type.boldHint': 'How a bold cell is drawn.',
      'type.boldWeight': 'Heavier face',
      'type.boldBright': 'Brighter color',
      'type.boldBoth': 'Both',
      'type.ligatures': 'Ligatures',
      'type.ligaturesHint': 'Shape sequences such as != and =>.',
      'type.detecting': 'Detecting',

      'window.title': 'Window',
      'window.opacity': 'Background opacity',
      'window.opacityHint': 'Fully opaque hides whatever is behind the terminal.',
      'window.cursor': 'Cursor',
      'window.block': 'Block',
      'window.bar': 'Bar',
      'window.underline': 'Underline',
      'window.blockHollow': 'Hollow block',
      'window.blink': 'Blink',
      'window.blinkHint': 'Keep the insertion point easy to find.',

      'reset.title': 'Reset everything',
      'reset.hint': 'Drops every override and returns to what this product ships.',
      'reset.action': 'Reset'
    },
    'zh-CN': {
      'app.title': '终端',
      'app.apply': '应用',
      'app.state.loading': '加载中',
      'app.state.clean': '没有改动',
      'app.state.dirty': '有未保存的改动',
      'app.state.unavailable': '设置不可用',

      'general.title': '通用',
      'general.language': '语言',
      'general.languageHint': '仅作用于本页面,暂不保存。',
      'general.systemDefault': '跟随系统',

      'appearance.title': '外观',
      'appearance.mode': '模式',
      'appearance.modeHint': '终端使用哪一套配色。',
      'appearance.system': '跟随系统',
      'appearance.light': '浅色',
      'appearance.dark': '深色',
      'appearance.scheme': '配色方案',
      'appearance.schemeHint': '悬停即可在所有已打开的终端上预览。浅色与深色分别选择。',
      'appearance.empty': '还没有配色方案,可在下方导入。',
      'appearance.import': '导入配色',
      'appearance.importHint': '支持 Windows Terminal JSON、Xresources 或 kitty conf。',
      'appearance.choose': '选择文件',

      'type.title': '字体',
      'type.families': '候选字体',
      'type.familiesHint': '取本机已安装的第一个。',
      'type.installed': '已安装字体',
      'type.installedHint': '追加到上面的列表。',
      'type.add': '添加字体',
      'type.size': '字号',
      'type.lineHeight': '行高',
      'type.lineHeightHint': '相对字号的倍数。',
      'type.bold': '粗体',
      'type.boldHint': '粗体单元格如何绘制。',
      'type.boldWeight': '更粗的字面',
      'type.boldBright': '更亮的颜色',
      'type.boldBoth': '两者兼用',
      'type.ligatures': '连字',
      'type.ligaturesHint': '将 != 和 => 这类序列合并显示。',
      'type.detecting': '检测中',

      'window.title': '窗口',
      'window.opacity': '背景不透明度',
      'window.opacityHint': '完全不透明会挡住终端背后的内容。',
      'window.cursor': '光标',
      'window.block': '方块',
      'window.bar': '竖线',
      'window.underline': '下划线',
      'window.blockHollow': '空心方块',
      'window.blink': '闪烁',
      'window.blinkHint': '让插入点更容易找到。',

      'reset.title': '全部重置',
      'reset.hint': '丢弃所有自定义,回到本产品出厂的设置。',
      'reset.action': '重置'
    }
  };

  var LOCALE_STORAGE_KEY = 'lingxia.webui.locale';

  function normalizeLocale(value) {
    // Hosts hand this over in several shapes ("zh-CN", "zh_Hans_CN", "en_CN").
    // Only the language subtag decides, so an underscore or a region that does
    // not match the language cannot drop the whole string on the floor.
    var tag = String(value || '').replace(/_/g, '-');
    if (/^zh(?:-|$)/i.test(tag)) return 'zh-CN';
    if (/^en(?:-|$)/i.test(tag)) return 'en-US';
    return null;
  }

  function storedLocale() {
    try {
      return normalizeLocale(global.localStorage.getItem(LOCALE_STORAGE_KEY));
    } catch (_) {
      return null;
    }
  }

  function systemLocale() {
    var candidates = Array.isArray(navigator.languages) && navigator.languages.length
      ? navigator.languages
      : [navigator.language || 'en-US'];
    return /^zh(?:-|$)/i.test(String(candidates[0] || ''))
      ? 'zh-CN'
      : 'en-US';
  }

  function resolveLocale() {
    return storedLocale() || systemLocale();
  }

  var locale = resolveLocale();

  function interpolate(value, variables) {
    return String(value).replace(/\{([a-zA-Z0-9_]+)\}/g, function (_, key) {
      return variables && Object.prototype.hasOwnProperty.call(variables, key)
        ? String(variables[key])
        : '{' + key + '}';
    });
  }

  function t(key, variables) {
    var active = dictionaries[locale] || dictionaries['en-US'];
    var value = active[key];
    if (value === undefined) value = dictionaries['en-US'][key];
    return interpolate(value === undefined ? key : value, variables);
  }

  function apply(root) {
    var scope = root || document;
    document.documentElement.lang = locale === 'zh-CN' ? 'zh-Hans' : 'en';
    scope.querySelectorAll('[data-i18n]').forEach(function (node) {
      node.textContent = t(node.getAttribute('data-i18n'));
    });
    [
      ['data-i18n-placeholder', 'placeholder'],
      ['data-i18n-title', 'title'],
      ['data-i18n-aria-label', 'aria-label']
    ].forEach(function (mapping) {
      scope.querySelectorAll('[' + mapping[0] + ']').forEach(function (node) {
        node.setAttribute(mapping[1], t(node.getAttribute(mapping[0])));
      });
    });
  }

  function setLocale(value) {
    var next = normalizeLocale(value);
    if (!next) return locale;
    locale = next;
    api.locale = locale;
    try {
      global.localStorage.setItem(LOCALE_STORAGE_KEY, locale);
    } catch (_) {}
    apply();
    return locale;
  }

  function useSystemLocale() {
    try {
      global.localStorage.removeItem(LOCALE_STORAGE_KEY);
    } catch (_) {}
    locale = systemLocale();
    api.locale = locale;
    apply();
    return locale;
  }

  var api = {
    locale: locale,
    t: t,
    apply: apply,
    setLocale: setLocale,
    useSystemLocale: useSystemLocale,
    storageKey: LOCALE_STORAGE_KEY
  };
  global.LingXiaI18n = api;

  function syncLocaleFromHost() {
    var bridge = global.LingXiaBridge;
    if (!bridge || typeof bridge.invoke !== 'function') return;
    function adoptHostLocale(result) {
      if (result && result.language == null) {
        var previousLocale = locale;
        var hadStoredLocale = !!storedLocale();
        useSystemLocale();
        if ((hadStoredLocale || previousLocale !== locale) &&
            global.location && typeof global.location.reload === 'function') {
          global.location.reload();
        }
        return;
      }
      var hostLocale = normalizeLocale(result && result.language);
      if (!hostLocale || hostLocale === locale) return;
      setLocale(hostLocale);
      // Reload only when the locale actually persisted; if localStorage is
      // unavailable the mismatch would survive the reload and loop forever,
      // so keep the in-memory apply() from setLocale instead.
      if (storedLocale() === hostLocale &&
          global.location && typeof global.location.reload === 'function') {
        global.location.reload();
      }
    }
    function refreshFromHost() {
      bridge.invoke('settings.getLanguage').then(adoptHostLocale, function () {});
    }
    var languageWatchRetryMs = 1000;
    var languageWatchRetryTimer = null;
    function scheduleLanguageWatch() {
      if (languageWatchRetryTimer != null) return;
      languageWatchRetryTimer = global.setTimeout(function () {
        languageWatchRetryTimer = null;
        attachLanguageWatch();
      }, languageWatchRetryMs);
      languageWatchRetryMs = Math.min(languageWatchRetryMs * 2, 30000);
    }
    function attachLanguageWatch() {
      if (typeof bridge.stream !== 'function') return;
      var watch = bridge.stream('settings.watchLanguage');
      var startedAt = Date.now();
      api.languageWatch = watch;
      watch.onEvent(adoptHostLocale);
      watch.onError(function () {
        if (api.languageWatch !== watch) return;
        api.languageWatch = null;
        // Transport reset: re-sync immediately, then reconnect with backoff.
        // Only a stream that stayed healthy for a while resets the backoff —
        // the seed event must not, or a flapping stream reconnects at the
        // floor forever.
        if (Date.now() - startedAt > 30000) languageWatchRetryMs = 1000;
        refreshFromHost();
        scheduleLanguageWatch();
      });
    }
    refreshFromHost();
    attachLanguageWatch();
  }

  if (typeof global.addEventListener === 'function') {
    global.addEventListener('storage', function (event) {
      if (event.key !== LOCALE_STORAGE_KEY) return;
      var next = normalizeLocale(event.newValue) || resolveLocale();
      if (next === locale) return;
      locale = next;
      api.locale = locale;
      if (global.location && typeof global.location.reload === 'function') {
        global.location.reload();
      } else {
        apply();
      }
    });
  }

  // Translate what is already in the markup. The browser package leaves this
  // to each page's own script; a settings screen with no logic worker has no
  // such script, and untranslated markup on first paint is the whole bug.
  if (typeof document !== 'undefined') {
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', function () { apply(); });
    } else {
      apply();
    }
  }

  syncLocaleFromHost();
})(window);
