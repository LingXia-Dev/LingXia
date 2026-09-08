(function (global) {
  'use strict';

  /*
   * Same contract as the browser package's i18n: `data-i18n` attributes in the
   * markup, a two-locale dictionary, and the product's language followed live
   * through the bridge. Sharing the shape matters more than sharing the file -
   * a settings screen that picked its own convention would be the one surface a
   * translator has to learn twice.
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

      'appearance.title': 'Appearance',
      'appearance.scheme': 'Color scheme',
      'appearance.schemeHint': "The product's light/dark setting chooses which of these pairs is shown. Select one to preview it.",
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

      'appearance.title': '外观',
      'appearance.scheme': '配色方案',
      'appearance.schemeHint': '产品的浅色/深色设置决定显示哪一套配色，点击即可预览。',
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

  function normalizeLocale(value) {
    // Hosts hand this over in several shapes ("zh-CN", "zh_Hans_CN", "en_CN").
    // Only the language subtag decides, so an underscore or a region that does
    // not match the language cannot drop the whole string on the floor.
    var tag = String(value || '').replace(/_/g, '-');
    if (/^zh(?:-|$)/i.test(tag)) return 'zh-CN';
    if (/^en(?:-|$)/i.test(tag)) return 'en-US';
    return null;
  }

  // The product owns the language; this screen only narrows it to a catalog it
  // actually ships. `navigator` is the fallback for a document opened outside a
  // host, never a preference of its own.
  function hostLocale() {
    var bridge = global.LingXiaBridge;
    if (bridge && bridge.displayLanguage) {
      var narrowed = normalizeLocale(bridge.displayLanguage.get());
      if (narrowed) return narrowed;
    }
    var candidates = Array.isArray(navigator.languages) && navigator.languages.length
      ? navigator.languages
      : [navigator.language || 'en-US'];
    return normalizeLocale(candidates[0]) || 'en-US';
  }

  var locale = hostLocale();

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

  // Re-apply in place rather than reload: this document was injected with the
  // bridge configuration, and navigating to the base URL fetches the raw file
  // without it, leaving a screen whose Logic calls are never answered.
  function adoptHostLocale() {
    var next = hostLocale();
    if (next === locale) return;
    locale = next;
    api.locale = locale;
    apply();
  }

  var api = {
    locale: locale,
    t: t,
    apply: apply
  };
  global.LingXiaI18n = api;

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

  if (global.LingXiaBridge && global.LingXiaBridge.displayLanguage) {
    global.LingXiaBridge.displayLanguage.subscribe(adoptHostLocale);
  }
})(window);
