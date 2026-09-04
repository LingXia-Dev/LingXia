(function (global) {
  'use strict';

  /*
   * Same contract as the browser package's i18n: `data-i18n` attributes in the
   * markup, a two-locale dictionary, and the host's language followed live
   * through replicated Logic state. Sharing the shape matters more than
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
      'app.state.applying': 'Applying…',
      'app.state.resetting': 'Resetting…',
      'app.state.unavailable': 'Settings unavailable',
      'app.applied': 'Applied to every open terminal',
      'app.conflict': 'Settings changed elsewhere. Reloaded the latest values.',
      'app.externalChange': 'Terminal settings changed elsewhere. Apply will reload the latest values.',

      'general.title': 'General',
      'general.language': 'Language',
      'general.languageHint': 'Only changes this screen. The app decides the rest.',
      'general.followApp': 'Same as app',

      'appearance.title': 'Appearance',
      'appearance.mode': 'Mode',
      'appearance.modeHint': 'Which scheme a terminal uses.',
      'appearance.system': 'System',
      'appearance.light': 'Light',
      'appearance.dark': 'Dark',
      'appearance.scheme': 'Color scheme',
      'appearance.schemeHint': 'Mode chooses whether light or dark schemes are shown. Select one to preview it.',
      'appearance.empty': 'No schemes yet. Import one below.',
      'appearance.import': 'Import a scheme',
      'appearance.importHint': 'Choose a compatible color-scheme file.',
      'appearance.choose': 'Choose file…',
      'appearance.builtIn': 'Built in',
      'appearance.imported': 'Imported',

      'type.title': 'Type',
      'type.family': 'Font family',
      'type.familyHint': 'Installed monospaced fonts.',
      'type.size': 'Size',
      'type.lineHeight': 'Line height',
      'type.lineHeightHint': "Multiplier for the font's natural line height.",
      'type.ligatures': 'Ligatures',
      'type.ligaturesHint': 'Shape sequences such as != and =>.',

      'windows.title': 'Windows',
      'windows.inlineImages': 'Inline images',
      'windows.inlineImagesHint': "Download Microsoft's optional compatibility runtime so Kitty images reach the terminal unchanged. New tabs and panes use it immediately.",
      'windows.downloading': 'Downloading compatibility runtime…',
      'windows.downloadingAmount': 'Downloading {received} of {total}',
      'windows.enabled': 'Inline images enabled for new terminal sessions',
      'windows.disabled': 'Inline image compatibility disabled',
      'windows.failed': 'Could not change inline images: {message}',

      'reset.title': 'Reset everything',
      'reset.hint': 'Drops every override and returns to what this product ships.',
      'reset.action': 'Reset',
      'reset.done': 'Terminal settings reset',
      'reset.typeDone': 'Type reset',
      'reset.appearanceDone': 'Appearance reset'
    },
    'zh-CN': {
      'app.title': '终端',
      'app.apply': '应用',
      'app.state.loading': '加载中',
      'app.state.clean': '没有改动',
      'app.state.dirty': '有未保存的改动',
      'app.state.applying': '正在应用…',
      'app.state.resetting': '正在重置…',
      'app.state.unavailable': '设置不可用',
      'app.applied': '已应用到所有打开的终端',
      'app.conflict': '设置已在其他位置更改，已重新加载最新值。',
      'app.externalChange': '终端设置已在其他位置更改。应用时会重新加载最新值。',

      'general.title': '通用',
      'general.language': '语言',
      'general.languageHint': '仅更改本页面，其余部分由应用决定。',
      'general.followApp': '与应用一致',

      'appearance.title': '外观',
      'appearance.mode': '模式',
      'appearance.modeHint': '终端使用哪一套配色。',
      'appearance.system': '跟随系统',
      'appearance.light': '浅色',
      'appearance.dark': '深色',
      'appearance.scheme': '配色方案',
      'appearance.schemeHint': '模式决定显示浅色或深色配色，点击即可预览。',
      'appearance.empty': '还没有配色方案,可在下方导入。',
      'appearance.import': '导入配色',
      'appearance.importHint': '选择兼容的配色方案文件。',
      'appearance.choose': '选择文件…',
      'appearance.builtIn': '内置',
      'appearance.imported': '已导入',

      'type.title': '字体',
      'type.family': '字体',
      'type.familyHint': '本机已安装的等宽字体。',
      'type.size': '字号',
      'type.lineHeight': '行高',
      'type.lineHeightHint': '字体自然行高的倍数。',
      'type.ligatures': '连字',
      'type.ligaturesHint': '将 != 和 => 这类序列合并显示。',

      'windows.title': 'Windows',
      'windows.inlineImages': '内联图片',
      'windows.inlineImagesHint': '下载 Microsoft 可选兼容运行库，让 Kitty 图片完整传给终端。新标签页和面板立即生效。',
      'windows.downloading': '正在下载兼容运行库…',
      'windows.downloadingAmount': '已下载 {received} / {total}',
      'windows.enabled': '新终端会话已启用内联图片',
      'windows.disabled': '已关闭内联图片兼容支持',
      'windows.failed': '无法更改内联图片设置：{message}',

      'reset.title': '全部重置',
      'reset.hint': '丢弃所有自定义,回到本产品出厂的设置。',
      'reset.action': '重置',
      'reset.done': '终端设置已重置',
      'reset.typeDone': '字体设置已重置',
      'reset.appearanceDone': '外观设置已重置'
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
  var appLocale = null;

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
    // Anything that mirrors this markup has to re-read it. A styled select
    // keeps its own visible label, and translating the underlying options
    // fires no event of its own — the label would keep the previous
    // language while the list it opens shows the new one.
    if (typeof CustomEvent === 'function') {
      document.dispatchEvent(new CustomEvent('lx-i18n-applied'));
    }
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
    adoptAppLocale: function (value) {
      var next = normalizeLocale(value);
      if (!next) return locale;
      appLocale = next;
      if (storedLocale() || next === locale) return locale;
      locale = next;
      api.locale = locale;
      apply();
      return locale;
    },
    /// Drop this screen's own choice and take the app's language again. The
    /// host decides it, so this is not the same as the operating system's.
    followApp: function () {
      try {
        global.localStorage.removeItem(LOCALE_STORAGE_KEY);
      } catch (_) {}
      if (appLocale) return api.adoptAppLocale(appLocale);
      return useSystemLocale();
    },
    storageKey: LOCALE_STORAGE_KEY
  };
  global.LingXiaI18n = api;

  if (typeof global.addEventListener === 'function') {
    global.addEventListener('storage', function (event) {
      if (event.key !== LOCALE_STORAGE_KEY) return;
      var next = normalizeLocale(event.newValue) || resolveLocale();
      if (next === locale) return;
      locale = next;
      api.locale = locale;
      apply();
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
})(window);
