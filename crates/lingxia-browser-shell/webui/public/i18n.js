(function (global) {
  'use strict';

  var dictionaries = {
    'en-US': {
      'common.add': 'Add',
      'common.cancel': 'Cancel',
      'common.close': 'Close',
      'common.delete': 'Delete',
      'common.remove': 'Remove',
      'common.save': 'Save',
      'common.somethingWentWrong': 'Something went wrong',
      'bookmarks.title': 'Bookmarks',
      'bookmarks.newGroup': 'New Group',
      'bookmarks.groupName': 'Group name',
      'bookmarks.all': 'All',
      'bookmarks.allTitle': 'All Bookmarks',
      'bookmarks.pinned': 'Pinned',
      'bookmarks.other': 'Other',
      'bookmarks.otherTitle': 'Other Bookmarks',
      'bookmarks.add': 'Add Bookmark',
      'bookmarks.optionalTitle': 'Title (optional)',
      'bookmarks.pinHint': 'Pinned bookmarks appear as the icon grid at the top of the sidebar - your fastest entry points.',
      'bookmarks.emptyTitle': 'No bookmarks yet',
      'bookmarks.emptyCopy': 'Bookmark a page from the browser toolbar (⌘D), or use Add Bookmark above. Pin the ones you use most to put them at the top of the sidebar.',
      'bookmarks.moveAria': 'Move bookmark to group',
      'bookmarks.noGroup': 'No group',
      'bookmarks.renameGroup': 'Rename group',
      'bookmarks.deleteGroup': 'Delete group',
      'bookmarks.dragReorder': 'Drag to reorder',
      'bookmarks.pin': 'Pin',
      'bookmarks.unpin': 'Unpin',
      'bookmarks.pinSidebar': 'Pin to sidebar',
      'bookmarks.unpinSidebar': 'Unpin from sidebar',
      'bookmarks.moveGroup': 'Move to group',
      'bookmarks.rename': 'Rename',
      'bookmarks.countOne': '1 bookmark',
      'bookmarks.countMany': '{count} bookmarks',
      'bookmarks.groupDeleted': 'Group deleted - bookmarks kept',
      'bookmarks.added': 'Added to bookmarks',
      'bookmarks.alreadyAdded': 'Already in bookmarks',
      'bookmarks.pinnedToast': 'Pinned to sidebar',
      'bookmarks.unpinnedToast': 'Unpinned',
      'bookmarks.removedToast': 'Removed from bookmarks',
      'bookmarks.moreActions': 'More bookmark actions',
      'bookmarks.import': 'Import bookmarks',
      'bookmarks.export': 'Export bookmarks',
      'bookmarks.imported': 'Imported {count} bookmarks; skipped {skipped}',
      'bookmarks.exported': 'Exported {count} bookmarks to {fileName}',
      'bookmarks.importFailed': 'Could not import this bookmark file',
      'bookmarks.exportFailed': 'Could not export bookmarks',
      'history.title': 'History',
      'history.loading': 'Loading...',
      'history.search': 'Search history',
      'history.clearMenu': 'Clear History...',
      'history.emptyTitle': 'No history yet',
      'history.emptyCopy': 'Pages you visit will appear here.',
      'history.noMatches': 'No matching history',
      'history.noMatchesCopy': 'Try a different title, website, or URL.',
      'history.clearTitle': 'Clear all browsing history?',
      'history.clearCopy': 'This removes visited pages from History. Cookies, site data, and cached files are not affected.',
      'history.clear': 'Clear History',
      'history.remove': 'Remove from history',
      'history.countOne': '1 visited page',
      'history.countMany': '{count} visited pages',
      'history.cleared': '{count} history entries cleared',
      'history.today': 'Today',
      'history.yesterday': 'Yesterday',
      'downloads.title': 'Downloads',
      'downloads.clearAll': 'Clear all',
      'downloads.loading': 'Loading...',
      'downloads.emptyTitle': 'No downloads yet',
      'downloads.emptyCopy': 'Files you download will appear here',
      'downloads.pause': 'Pause',
      'downloads.paused': 'Paused',
      'downloads.resume': 'Resume',
      'downloads.retry': 'Retry',
      'downloads.cancelDownload': 'Cancel download',
      'downloads.showInFolder': 'Show in Finder',
      'downloads.openFile': 'Open {name}',
      'downloads.failed': 'Download failed',
      'downloads.removed': 'Removed',
      'downloads.downloading': 'Downloading',
      'downloads.earlier': 'Earlier',
      'downloads.activeCount': '{count} downloading',
      'downloads.fileCountOne': '1 file',
      'downloads.fileCountMany': '{count} files',
      'downloads.loadFailed': 'Failed to load',
      'settings.title': 'Settings',
      'settings.general': 'General',
      'settings.language': 'Language',
      'settings.languageCopy': 'Use your system language automatically, or choose a language for browser pages.',
      'settings.languageAuto': 'Automatic',
      'settings.languageSaveFailed': 'Language could not be saved.',
      'settings.downloads': 'Downloads',
      'settings.proxy': 'Proxy',
      'settings.privacy': 'Privacy',
      'settings.about': 'About',
      'settings.location': 'Location',
      'settings.changeLocation': 'Change location',
      'settings.change': 'Change',
      'settings.resetDefault': 'Reset to default',
      'settings.reset': 'Reset',
      'settings.default': 'Default',
      'settings.custom': 'Custom',
      'settings.proxyServer': 'Proxy Server',
      'settings.proxyServerCopy': 'Configure the SOCKS server used by both Always Proxy and Auto Switch modes.',
      'settings.socksHost': 'SOCKS host',
      'settings.socksHostCopy': 'Used by both Always Proxy and Auto Switch modes.',
      'settings.socksPort': 'SOCKS port',
      'settings.socksPortCopy': 'Default is 1080.',
      'settings.username': 'Username',
      'settings.usernameCopy': 'Optional SOCKS username.',
      'settings.password': 'Password',
      'settings.passwordCopy': 'Optional SOCKS password.',
      'settings.saveServerSettings': 'Save server settings',
      'settings.saveServerCopy': 'Save the server fields and reapply the current routing mode.',
      'settings.saveServer': 'Save Server',
      'settings.saved': 'Saved',
      'settings.routingMode': 'Routing Mode',
      'settings.routingCopy': 'Choose how browser traffic should be routed: direct, auto switch by rule list, or always through the upstream proxy.',
      'settings.loading': 'Loading',
      'settings.direct': 'Direct',
      'settings.directDesc': 'No proxy. Browser traffic goes out normally.',
      'settings.autoSwitch': 'Auto Switch',
      'settings.autoSwitchDesc': 'Use the rule list below. Matched requests go to the proxy server, default traffic stays direct.',
      'settings.alwaysProxy': 'Always Proxy',
      'settings.alwaysProxyDesc': 'Send all browser traffic to the configured SOCKS server.',
      'settings.directDetail': 'Browser traffic goes out normally. The proxy server configuration stays saved, but it is not used in this mode.',
      'settings.alwaysProxyDetail': 'All browser requests use the SOCKS server for newly created browser tabs.',
      'settings.switchRules': 'Switch Rules',
      'settings.switchRulesCopy': 'Auto Switch checks custom rules first, then the downloaded rule list, and finally falls back to direct.',
      'settings.matchHelp': 'Enter a host pattern to match that host and its subdomains.',
      'settings.conditionType': 'Condition Type',
      'settings.conditionDetails': 'Condition Details',
      'settings.profile': 'Profile',
      'settings.ruleListRules': 'Rule list rules',
      'settings.ruleListRulesCopy': 'Any request matching the configured rule list.',
      'settings.noCustomRules': 'No custom rules yet.',
      'settings.defaultRuleCopy': 'Requests that do not match the rule list.',
      'settings.addRule': 'Add Rule',
      'settings.ruleListConfig': 'Rule List Config',
      'settings.empty': 'Empty',
      'settings.ruleListUrl': 'Rule List URL',
      'settings.ruleListUrlCopy': 'HTTPS URL used when downloading the rule list.',
      'settings.lastUpdated': 'Last updated',
      'settings.never': 'Never',
      'settings.downloadRules': 'Download Rules Now',
      'settings.proxyHelper': 'Choose a mode, then save when you want to update the proxy configuration used by browser tabs.',
      'settings.matchSite': 'Match site',
      'settings.applying': 'Applying configuration...',
      'settings.invalidServer': 'Always Proxy and Auto Switch require a valid SOCKS5 host and port.',
      'settings.saveAfterEdit': 'Mode buttons update the selected configuration. Use "Save Server" after editing the SOCKS fields.',
      'settings.active': 'Active',
      'settings.unsupported': 'Unsupported',
      'settings.error': 'Error',
      'settings.pending': 'Pending',
      'settings.ready': 'Ready',
      'settings.rulesNotDownloaded': 'Rules have not been downloaded yet.',
      'settings.privacyBrowsingData': 'Browsing Data',
      'settings.browsingHistory': 'Browsing history',
      'settings.viewHistory': 'View History',
      'settings.cachedFiles': 'Cached images and files',
      'settings.cookies': 'Cookies and site data',
      'settings.clearBrowsingData': 'Clear browsing data',
      'settings.clearBrowsingDataCopy': 'Choose a time range and the data you want to remove.',
      'settings.clearBrowsingDataMenu': 'Clear Browsing Data...',
      'settings.clearDialogCopy': 'Select a time range and the information to remove from this browser profile.',
      'settings.timeRange': 'Time range',
      'settings.lastHour': 'Last hour',
      'settings.last24Hours': '24 hours',
      'settings.last7Days': '7 days',
      'settings.last4Weeks': '4 weeks',
      'settings.allTime': 'All time',
      'settings.dataToRemove': 'Data to remove',
      'settings.historyDataCopy': 'Visited pages and their timestamps.',
      'settings.cacheDataCopy': 'Sites may load more slowly on your next visit.',
      'settings.cookieDataCopy': 'Sign-in sessions, local storage, and databases.',
      'settings.cookieWarning': 'You will be signed out of most websites.',
      'settings.clearData': 'Clear Data',
      'settings.noSites': 'No sites',
      'settings.siteCountOne': '1 site',
      'settings.siteCountMany': '{count} sites',
      'settings.noSavedHistory': 'No saved history',
      'settings.cacheSites': '{sites} with cached files',
      'settings.nothingCached': 'Nothing cached',
      'settings.cookieUsage': '{count} cookies and site data from {sites}',
      'settings.noCookies': 'No cookies or site data',
      'settings.unavailable': 'Unavailable',
      'settings.dataCleared': 'Selected browsing data cleared.',
      'settings.clearFailed': 'Failed to clear browsing data.',
      'settings.clearSiteData': 'Clear data for this site',
      'settings.clearSiteDataCopy': 'Remove data saved by this website without affecting other sites.',
      'settings.clearSiteDataAction': 'Clear Site Data',
      'settings.siteCacheDataCopy': 'Only cache storage that can be isolated to this site is removed.',
      'settings.siteCookieDataCopy': 'Cookies, local storage, databases, and service workers for this website.',
      'settings.siteCookieWarning': 'You will be signed out of this website.',
      'settings.siteUnavailable': 'This website is no longer available.',
      'settings.siteDataCleared': 'Data for this website was cleared.',
      'settings.siteDataClearedSharedCacheKept': 'Site data was cleared. The shared network cache was kept.',
      'settings.clearSiteDataFailed': 'Could not clear data for this website.',
      'settings.appVersion': 'App Version',
      'settings.lingxiaVersion': 'LingXia Version',
      'settings.appVersionValue': 'App Version {version}',
      'settings.unknownProxyError': 'Unknown proxy error',
      'settings.settingsSaved': 'Settings saved.',
      'settings.savedForNewTabs': 'Saved. The configuration will apply to newly created browser tabs.',
      'settings.proxyActive': 'Proxy configuration is active.',
      'settings.proxyFailed': 'Proxy configuration failed.',
      'settings.hostRequired': 'SOCKS5 host is required for the selected mode.',
      'settings.portInvalid': 'SOCKS5 port must be between 1 and 65535.',
      'settings.serverSaved': 'Proxy server settings saved.',
      'settings.autoSaved': 'Auto Switch settings saved.',
      'settings.downloadingAutoRules': 'Downloading Auto Switch rules and applying the mode...',
      'settings.downloadingRules': 'Downloading rules from the configured source...',
      'newtab.title': 'New Tab',
      'newtab.searchPlaceholder': 'Search the web',
      'newtab.customize': 'Customize New Tab',
      'newtab.searchEngine': 'Search engine',
      'newtab.searchEngineHelp': 'Choose the engine used by the New Tab search box.',
      'newtab.defaultEngine': 'Default',
      'newtab.addEngine': 'Add search engine',
      'newtab.engineName': 'Name',
      'newtab.engineNamePlaceholder': 'Example Search',
      'newtab.engineUrl': 'Search URL',
      'newtab.engineUrlPlaceholder': 'https://example.com/search?q={query}',
      'newtab.engineUrlHelp': 'Use {query} where the search text belongs.',
      'newtab.invalidEngine': 'Enter a name and a valid HTTP or HTTPS URL containing {query}.',
      'newtab.duplicateEngine': 'A search engine with this URL already exists.',
      'newtab.background': 'Background',
      'newtab.backgroundHelp': 'Use a local image, or keep the default clean background.',
      'newtab.noBackground': 'No background image',
      'newtab.chooseImage': 'Choose image',
      'newtab.replaceImage': 'Replace image',
      'newtab.removeImage': 'Remove image',
      'newtab.imageTooLarge': 'Choose an image smaller than 25 MB.',
      'newtab.imageReadFailed': 'The selected image could not be loaded.',
      'newtab.settingsSaved': 'New Tab settings updated.'
    },
    'zh-CN': {
      'common.add': '添加',
      'common.cancel': '取消',
      'common.close': '关闭',
      'common.delete': '删除',
      'common.remove': '移除',
      'common.save': '保存',
      'common.somethingWentWrong': '出现错误',
      'bookmarks.title': '书签',
      'bookmarks.newGroup': '新建分组',
      'bookmarks.groupName': '分组名称',
      'bookmarks.all': '全部',
      'bookmarks.allTitle': '全部书签',
      'bookmarks.pinned': '已固定',
      'bookmarks.other': '其他',
      'bookmarks.otherTitle': '其他书签',
      'bookmarks.add': '添加书签',
      'bookmarks.optionalTitle': '标题（可选）',
      'bookmarks.pinHint': '固定的书签会显示在侧栏顶部的图标区域，方便快速打开。',
      'bookmarks.emptyTitle': '暂无书签',
      'bookmarks.emptyCopy': '可从浏览器工具栏添加书签（⌘D），也可使用上方的“添加书签”。将常用网站固定后，它们会显示在侧栏顶部。',
      'bookmarks.moveAria': '移动书签到分组',
      'bookmarks.noGroup': '未分组',
      'bookmarks.renameGroup': '重命名分组',
      'bookmarks.deleteGroup': '删除分组',
      'bookmarks.dragReorder': '拖动排序',
      'bookmarks.pin': '固定',
      'bookmarks.unpin': '取消固定',
      'bookmarks.pinSidebar': '固定到侧栏',
      'bookmarks.unpinSidebar': '从侧栏取消固定',
      'bookmarks.moveGroup': '移动到分组',
      'bookmarks.rename': '重命名',
      'bookmarks.countOne': '1 个书签',
      'bookmarks.countMany': '{count} 个书签',
      'bookmarks.groupDeleted': '分组已删除，书签已保留',
      'bookmarks.added': '已添加到书签',
      'bookmarks.alreadyAdded': '已在书签中',
      'bookmarks.pinnedToast': '已固定到侧栏',
      'bookmarks.unpinnedToast': '已取消固定',
      'bookmarks.removedToast': '已从书签移除',
      'bookmarks.moreActions': '更多书签操作',
      'bookmarks.import': '导入书签',
      'bookmarks.export': '导出书签',
      'bookmarks.imported': '已导入 {count} 个书签，跳过 {skipped} 个',
      'bookmarks.exported': '已将 {count} 个书签导出到 {fileName}',
      'bookmarks.importFailed': '无法导入此书签文件',
      'bookmarks.exportFailed': '无法导出书签',
      'history.title': '历史记录',
      'history.loading': '正在加载...',
      'history.search': '搜索历史记录',
      'history.clearMenu': '清除历史记录...',
      'history.emptyTitle': '暂无历史记录',
      'history.emptyCopy': '访问过的网页会显示在这里。',
      'history.noMatches': '没有匹配的历史记录',
      'history.noMatchesCopy': '请尝试其他标题、网站或网址。',
      'history.clearTitle': '清除全部浏览历史记录？',
      'history.clearCopy': '这会从历史记录中移除访问过的网页，不会影响 Cookie、网站数据和缓存文件。',
      'history.clear': '清除历史记录',
      'history.remove': '从历史记录中移除',
      'history.countOne': '访问过 1 个网页',
      'history.countMany': '访问过 {count} 个网页',
      'history.cleared': '已清除 {count} 条历史记录',
      'history.today': '今天',
      'history.yesterday': '昨天',
      'downloads.title': '下载内容',
      'downloads.clearAll': '全部清除',
      'downloads.loading': '正在加载...',
      'downloads.emptyTitle': '暂无下载内容',
      'downloads.emptyCopy': '下载的文件会显示在这里',
      'downloads.pause': '暂停',
      'downloads.paused': '已暂停',
      'downloads.resume': '继续',
      'downloads.retry': '重试',
      'downloads.cancelDownload': '取消下载',
      'downloads.showInFolder': '在访达中显示',
      'downloads.openFile': '打开 {name}',
      'downloads.failed': '下载失败',
      'downloads.removed': '已移除',
      'downloads.downloading': '正在下载',
      'downloads.earlier': '更早',
      'downloads.activeCount': '{count} 个正在下载',
      'downloads.fileCountOne': '1 个文件',
      'downloads.fileCountMany': '{count} 个文件',
      'downloads.loadFailed': '加载失败',
      'settings.title': '设置',
      'settings.general': '通用',
      'settings.language': '语言',
      'settings.languageCopy': '自动跟随系统语言，或为浏览器页面手动选择语言。',
      'settings.languageAuto': '自动',
      'settings.languageSaveFailed': '无法保存语言设置。',
      'settings.downloads': '下载',
      'settings.proxy': '代理',
      'settings.privacy': '隐私',
      'settings.about': '关于',
      'settings.location': '保存位置',
      'settings.changeLocation': '更改保存位置',
      'settings.change': '更改',
      'settings.resetDefault': '恢复默认位置',
      'settings.reset': '恢复',
      'settings.default': '默认',
      'settings.custom': '自定义',
      'settings.proxyServer': '代理服务器',
      'settings.proxyServerCopy': '配置“始终使用代理”和“自动切换”模式共用的 SOCKS 服务器。',
      'settings.socksHost': 'SOCKS 主机',
      'settings.socksHostCopy': '供“始终使用代理”和“自动切换”模式使用。',
      'settings.socksPort': 'SOCKS 端口',
      'settings.socksPortCopy': '默认端口为 1080。',
      'settings.username': '用户名',
      'settings.usernameCopy': '可选的 SOCKS 用户名。',
      'settings.password': '密码',
      'settings.passwordCopy': '可选的 SOCKS 密码。',
      'settings.saveServerSettings': '保存服务器设置',
      'settings.saveServerCopy': '保存服务器字段，并重新应用当前路由模式。',
      'settings.saveServer': '保存服务器',
      'settings.saved': '已保存',
      'settings.routingMode': '路由模式',
      'settings.routingCopy': '选择浏览器流量直连、按规则自动切换或始终使用上游代理。',
      'settings.loading': '正在加载',
      'settings.direct': '直连',
      'settings.directDesc': '不使用代理，浏览器流量正常直连。',
      'settings.autoSwitch': '自动切换',
      'settings.autoSwitchDesc': '使用下方规则列表，匹配的请求走代理，其余流量保持直连。',
      'settings.alwaysProxy': '始终使用代理',
      'settings.alwaysProxyDesc': '所有浏览器流量都发送到已配置的 SOCKS 服务器。',
      'settings.directDetail': '浏览器流量正常直连。代理服务器配置会保留，但此模式不会使用。',
      'settings.alwaysProxyDetail': '新建浏览器标签页的全部请求都会使用 SOCKS 服务器。',
      'settings.switchRules': '切换规则',
      'settings.switchRulesCopy': '自动切换会先检查自定义规则，再检查下载的规则列表，最后回退为直连。',
      'settings.matchHelp': '输入主机模式以匹配该主机及其子域名。',
      'settings.conditionType': '条件类型',
      'settings.conditionDetails': '条件详情',
      'settings.profile': '策略',
      'settings.ruleListRules': '规则列表',
      'settings.ruleListRulesCopy': '所有与已配置规则列表匹配的请求。',
      'settings.noCustomRules': '暂无自定义规则。',
      'settings.defaultRuleCopy': '未匹配规则列表的请求。',
      'settings.addRule': '添加规则',
      'settings.ruleListConfig': '规则列表配置',
      'settings.empty': '空',
      'settings.ruleListUrl': '规则列表网址',
      'settings.ruleListUrlCopy': '下载规则列表时使用的 HTTPS 网址。',
      'settings.lastUpdated': '上次更新',
      'settings.never': '从未',
      'settings.downloadRules': '立即下载规则',
      'settings.proxyHelper': '选择模式，并在需要更新浏览器标签页使用的代理配置时保存。',
      'settings.matchSite': '匹配网站',
      'settings.applying': '正在应用配置...',
      'settings.invalidServer': '始终使用代理和自动切换需要有效的 SOCKS5 主机与端口。',
      'settings.saveAfterEdit': '模式按钮会更新所选配置。编辑 SOCKS 字段后请使用“保存服务器”。',
      'settings.active': '已启用',
      'settings.unsupported': '不支持',
      'settings.error': '错误',
      'settings.pending': '处理中',
      'settings.ready': '就绪',
      'settings.rulesNotDownloaded': '尚未下载规则。',
      'settings.privacyBrowsingData': '浏览数据',
      'settings.browsingHistory': '浏览历史记录',
      'settings.viewHistory': '查看历史记录',
      'settings.cachedFiles': '缓存的图片和文件',
      'settings.cookies': 'Cookie 和网站数据',
      'settings.clearBrowsingData': '清除浏览数据',
      'settings.clearBrowsingDataCopy': '选择时间范围以及要移除的数据。',
      'settings.clearBrowsingDataMenu': '清除浏览数据...',
      'settings.clearDialogCopy': '选择要从此浏览器配置中移除的信息和时间范围。',
      'settings.timeRange': '时间范围',
      'settings.lastHour': '过去 1 小时',
      'settings.last24Hours': '过去 24 小时',
      'settings.last7Days': '过去 7 天',
      'settings.last4Weeks': '过去 4 周',
      'settings.allTime': '全部时间',
      'settings.dataToRemove': '要移除的数据',
      'settings.historyDataCopy': '访问过的网页及其时间。',
      'settings.cacheDataCopy': '下次访问时，网站的加载速度可能会变慢。',
      'settings.cookieDataCopy': '登录会话、本地存储和数据库。',
      'settings.cookieWarning': '大多数网站会退出登录。',
      'settings.clearData': '清除数据',
      'settings.noSites': '无网站',
      'settings.siteCountOne': '1 个网站',
      'settings.siteCountMany': '{count} 个网站',
      'settings.noSavedHistory': '没有保存的历史记录',
      'settings.cacheSites': '{sites}有缓存文件',
      'settings.nothingCached': '没有缓存',
      'settings.cookieUsage': '{count} 个 Cookie，网站数据来自 {sites}',
      'settings.noCookies': '没有 Cookie 或网站数据',
      'settings.unavailable': '不可用',
      'settings.dataCleared': '已清除所选浏览数据。',
      'settings.clearFailed': '清除浏览数据失败。',
      'settings.clearSiteData': '清除此网站的数据',
      'settings.clearSiteDataCopy': '仅移除此网站保存的数据，不影响其他网站。',
      'settings.clearSiteDataAction': '清除网站数据',
      'settings.siteCacheDataCopy': '仅移除能够与其他网站隔离的缓存数据。',
      'settings.siteCookieDataCopy': '此网站的 Cookie、本地存储、数据库和 Service Worker。',
      'settings.siteCookieWarning': '你将退出此网站的登录状态。',
      'settings.siteUnavailable': '此网站已不可用。',
      'settings.siteDataCleared': '已清除此网站的数据。',
      'settings.siteDataClearedSharedCacheKept': '已清除网站数据；共享网络缓存已保留。',
      'settings.clearSiteDataFailed': '无法清除此网站的数据。',
      'settings.appVersion': '应用版本',
      'settings.lingxiaVersion': 'LingXia 版本',
      'settings.appVersionValue': '应用版本 {version}',
      'settings.unknownProxyError': '未知代理错误',
      'settings.settingsSaved': '设置已保存。',
      'settings.savedForNewTabs': '已保存。该配置将应用于新建的浏览器标签页。',
      'settings.proxyActive': '代理配置已启用。',
      'settings.proxyFailed': '代理配置失败。',
      'settings.hostRequired': '所选模式需要 SOCKS5 主机。',
      'settings.portInvalid': 'SOCKS5 端口必须在 1 到 65535 之间。',
      'settings.serverSaved': '代理服务器设置已保存。',
      'settings.autoSaved': '自动切换设置已保存。',
      'settings.downloadingAutoRules': '正在下载自动切换规则并应用模式...',
      'settings.downloadingRules': '正在从配置的来源下载规则...',
      'newtab.title': '新标签页',
      'newtab.searchPlaceholder': '搜索网络',
      'newtab.customize': '自定义新标签页',
      'newtab.searchEngine': '搜索引擎',
      'newtab.searchEngineHelp': '选择新标签页搜索框使用的搜索引擎。',
      'newtab.defaultEngine': '默认',
      'newtab.addEngine': '添加搜索引擎',
      'newtab.engineName': '名称',
      'newtab.engineNamePlaceholder': '示例搜索',
      'newtab.engineUrl': '搜索网址',
      'newtab.engineUrlPlaceholder': 'https://example.com/search?q={query}',
      'newtab.engineUrlHelp': '请用 {query} 表示搜索内容的位置。',
      'newtab.invalidEngine': '请输入名称，以及包含 {query} 的有效 HTTP 或 HTTPS 网址。',
      'newtab.duplicateEngine': '该搜索网址已经存在。',
      'newtab.background': '背景',
      'newtab.backgroundHelp': '可选择本地图片，也可保持默认的简洁背景。',
      'newtab.noBackground': '无背景图片',
      'newtab.chooseImage': '选择图片',
      'newtab.replaceImage': '更换图片',
      'newtab.removeImage': '移除图片',
      'newtab.imageTooLarge': '请选择小于 25 MB 的图片。',
      'newtab.imageReadFailed': '无法加载所选图片。',
      'newtab.settingsSaved': '新标签页设置已更新。'
    }
  };

  var LOCALE_STORAGE_KEY = 'lingxia.webui.locale';

  function normalizeLocale(value) {
    if (value === 'zh-CN' || /^zh(?:-|$)/i.test(String(value || ''))) return 'zh-CN';
    if (value === 'en-US' || /^en(?:-|$)/i.test(String(value || ''))) return 'en-US';
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
    function attachLanguageWatch() {
      if (typeof bridge.stream !== 'function') return;
      var watch = bridge.stream('settings.watchLanguage');
      api.languageWatch = watch;
      watch.onEvent(adoptHostLocale);
      watch.onError(function () {
        if (api.languageWatch !== watch) return;
        // Transport reset: re-sync then re-subscribe so changes keep applying.
        refreshFromHost();
        attachLanguageWatch();
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

  syncLocaleFromHost();
})(window);
