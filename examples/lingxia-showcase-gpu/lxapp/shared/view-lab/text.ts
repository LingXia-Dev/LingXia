// The View catalogue's words, in both display languages: its groups, each demo's stable id for specs, its
// title and the one sentence saying what to try, and what each group does not show yet.
import { resolveDisplayLanguage } from '../display-language';

export type Words = { en: string; zh: string };
export type GroupId = 'text' | 'gestures' | 'overlays' | 'accessibility' | 'motion';

export const groups: readonly { id: GroupId; page: string; title: Words; line: Words }[] = [
  { id: 'text', page: 'view-text', title: { en: 'Text & input', zh: '文字与输入' }, line: { en: 'Paragraphs, fields and keys', zh: '段落、输入框与按键' } },
  { id: 'gestures', page: 'view-gestures', title: { en: 'Gestures & pointer', zh: '手势与指针' }, line: { en: 'Press, drag and swipe', zh: '按压、拖动与滑动' } },
  { id: 'overlays', page: 'view-overlays', title: { en: 'Overlays', zh: '浮层' }, line: { en: 'Popovers and menus against what opens them', zh: '贴着触发处打开的浮层与菜单' } },
  { id: 'accessibility', page: 'view-accessibility', title: { en: 'Accessibility', zh: '无障碍' }, line: { en: 'Keyboard groups a screen reader can follow', zh: '读屏软件能跟随的键盘分组' } },
  { id: 'motion', page: 'view-motion', title: { en: 'Motion', zh: '动效' }, line: { en: 'Boxes that change size and place smoothly', zh: '平滑改变大小与位置的方块' } },
];

export const demos = {
  text: {
    paragraphs: { testId: 'lab-text', title: { en: 'Paragraphs', zh: '段落' }, try: { en: 'Numbers line up in columns, line breaks are kept where asked, and a long address breaks anywhere.', zh: '数字按列对齐，换行按要求保留，长网址可在任意处断开。' } },
    runs: { testId: 'lab-rich-text', title: { en: 'Mixed runs and links', zh: '混排与链接' }, try: { en: 'Tap the link inside the sentence: the count goes up.', zh: '点按句中的链接，计数加一。' } },
    document: { testId: 'lab-document', title: { en: 'A long document', zh: '长文档' }, try: { en: 'Headings, clamped previews and a two-column list of terms from one set of data.', zh: '同一份数据排出标题、截断的摘要和两列的术语表。' } },
    selecting: { testId: 'lab-selection', title: { en: 'Selecting text', zh: '选择文字' }, try: { en: 'Long-press or double-click a word, then drag to extend and copy.', zh: '长按或双击一个词，再拖动扩展选区并复制。' } },
    suggestions: { testId: 'lab-type', title: { en: 'Suggestions while typing', zh: '输入时的建议' }, try: { en: 'Type to filter. Arrows move, Enter chooses, Escape closes; the list opens against the field.', zh: '输入以筛选。方向键移动，回车选择，Esc 关闭；列表贴着输入框打开。' } },
    partly: { testId: 'lab-partly', title: { en: 'A box that is partly checked', zh: '部分选中的复选框' }, try: { en: 'Check one topping: "All toppings" shows a dash. Tap it and every topping is checked.', zh: '勾选一种配料，“全部配料”显示一条横线；点它则全部勾选。' } },
    linkOff: { testId: 'lab-link-off', title: { en: 'A link that is switched off', zh: '已停用的链接' }, try: { en: 'The first link does nothing and reads as unavailable; the second counts its taps.', zh: '第一个链接点了没有反应，读屏软件读作不可用；第二个会计数。' } },
    form: { testId: 'lab-form', title: { en: 'Required, invalid and described fields', zh: '必填、无效与带说明的字段' }, try: { en: 'Each field tells a screen reader its hint and its error.', zh: '每个字段都把提示和错误告诉读屏软件。' } },
  },
  gestures: {
    pointer: { testId: 'lab-scrub', title: { en: 'Where the pointer is', zh: '指针的位置' }, try: { en: 'Drag across the box to move the marker; drag up or down and the page scrolls.', zh: '在方块上横向拖动以移动标记；上下拖动则滚动页面。' } },
    stops: { testId: 'lab-drag', title: { en: 'Drag between stops', zh: '在停靠点之间拖动' }, try: { en: 'Drag the card and let go: it settles on the nearest stop.', zh: '拖动卡片后松手，它停在最近的停靠点。' } },
    chain: { testId: 'lab-chain', title: { en: 'Drag inside a scroll', zh: '滚动区里的拖动' }, try: { en: 'Pull the panel up: it opens first, then its list scrolls.', zh: '向上拉面板：先展开，随后列表才滚动。' } },
    sheet: { testId: 'lab-detents', title: { en: 'A sheet with stops', zh: '带停靠点的面板' }, try: { en: 'Open the sheet, pull it to full height, flick it down to close.', zh: '打开面板，拉到全高，向下轻扫关闭。' } },
    rows: { testId: 'lab-swipe', title: { en: 'Swipe a row for actions', zh: '滑动行显示操作' }, try: { en: 'Swipe a row left for Archive and Delete; a full swipe deletes.', zh: '向左滑动一行显示归档和删除；滑到底即删除。' } },
    pages: { testId: 'lab-pager', title: { en: 'Pages you swipe between', zh: '滑动翻页' }, try: { en: 'Swipe: one page at a time; the buttons do the same.', zh: '滑动一次翻一页；按钮效果相同。' } },
    small: { testId: 'lab-small', title: { en: 'A small control that is easy to hit', zh: '容易点中的小控件' }, try: { en: 'Tap just outside the small square: it still counts, because a finger gets a full-size target.', zh: '点小方块的外沿也会计数：手指得到的是完整大小的触控区。' } },
    nested: { testId: 'lab-nested', title: { en: 'A control inside a row', zh: '行里的控件' }, try: { en: 'Tap the switch: only the switch changes. Tap the row: the row is pressed.', zh: '点开关只切换开关；点行则按下整行。' } },
  },
  overlays: {
    menu: { testId: 'lab-menu', title: { en: 'A menu you ask for', zh: '按需唤出的菜单' }, try: { en: 'Hold a finger on the row, or click it with the secondary mouse button: its menu opens. A tap does nothing.', zh: '长按这一行，或用鼠标右键点击：菜单打开。轻点没有反应。' } },
    submenu: { testId: 'lab-submenu', title: { en: 'A menu inside a menu', zh: '菜单里的菜单' }, try: { en: 'Open the menu and choose "Move to": a second menu opens beside it and the first stays. Choose a folder and both close. With a keyboard, Right opens it and Left closes it.', zh: '打开菜单并选择“移动到”：第二个菜单在旁边打开，第一个保持不动；选一个文件夹后两个都关闭。使用键盘时，右方向键打开，左方向键关闭。' } },
    beside: { testId: 'lab-beside', title: { en: 'A popover beside what opens it', zh: '在触发处旁边打开的浮层' }, try: { en: 'Each button opens the note at its side. With no room there it opens on the other side, and under a wide button.', zh: '每个按钮都在自己旁边打开提示；那一侧放不下就换到另一侧，按钮太宽则在其下方。' } },
  },
  accessibility: {
    days: { testId: 'lab-days', title: { en: 'A grid of days', zh: '日期网格' }, try: { en: 'With a keyboard: Tab reaches the grid once; arrows move by day and by week, Home and End go to the week\'s ends, PageUp and PageDown change the month.', zh: '使用键盘：Tab 只进入网格一次；方向键按天、按周移动，Home 和 End 到本周两端，PageUp 和 PageDown 切换月份。' } },
  },
  motion: {
    panel: { testId: 'lab-panel', title: { en: 'A panel that opens and closes', zh: '展开与收起的面板' }, try: { en: 'Tap the heading: the panel grows to show its text and the row below moves down with it. Tap again and it closes.', zh: '点按标题：面板展开露出文字，下面的一行随之下移；再点一次收起。' } },
  },
} as const satisfies Record<GroupId, Record<string, { testId: string; title: Words; try: Words }>>;

/** What View provides in a group and the catalogue has no demo of yet. */
export const missing: Record<GroupId, readonly Words[]> = {
  text: [
    { en: 'Fields by type (number, date, time, search) and moving between them', zh: '各类输入框（数字、日期、时间、搜索）及其间的移动' },
    { en: 'Password field', zh: '密码输入框' },
    { en: 'Following the system text size', zh: '跟随系统字号' },
    { en: 'Placeholder colour', zh: '占位文字颜色' },
  ],
  overlays: [
    { en: 'Dialog and sheet', zh: '对话框与底部面板' },
    { en: 'Hint on hover or focus', zh: '悬停或聚焦时的提示' },
    { en: 'Date and time pickers', zh: '日期与时间选择器' },
  ],
  accessibility: [
    { en: 'Names and roles of each control', zh: '各控件的名称与角色' },
    { en: 'Tabs, a toolbar and a tree', zh: '标签页、工具栏与树' },
    { en: 'Announcements when something changes', zh: '内容变化时的播报' },
  ],
  motion: [
    { en: 'Fading and sliding in and out', zh: '淡入淡出与滑入滑出' },
    { en: 'Rows that move when their order changes', zh: '顺序变化时移动的行' },
    { en: 'Springs', zh: '弹簧动效' },
    { en: 'Less motion when the system asks for it', zh: '系统要求减弱动效时的表现' },
  ],
  gestures: [
    { en: 'Pull to refresh', zh: '下拉刷新' },
    { en: 'Back from the screen edge', zh: '屏幕边缘返回' },
    { en: 'Slider press', zh: '滑块的按下状态' },
  ],
};

export const chrome = {
  index: { en: 'What a page is built from.', zh: '页面由这些构成。' },
  missing: { en: 'Not shown yet', zh: '尚未展示' },
} as const;

/** How many demos a group holds, as the index says it. */
export const count = (n: number): Words => ({ en: n === 1 ? '1 demo' : `${n} demos`, zh: `${n} 个示例` });

const none = (value: string) => (value === 'none' ? '无' : value);
const yes = (value: boolean): Words => (value ? { en: 'open', zh: '已打开' } : { en: 'closed', zh: '已关闭' });

/** Each demo's state in words; the same values a spec reads from the readout's `data-state`. */
export const readouts = {
  scrub: (box: string | null, at: number, last: string): Words => ({
    en: `${box ? `Box ${box} · marker` : 'Marker'} at ${at} · last: ${last}`,
    zh: `${box ? `方块 ${box} · ` : ''}标记在 ${at} · 上次：${none(last)}`,
  }),
  drag: (at: number, last: string): Words => ({ en: `Stop ${at} · last: ${last}`, zh: `停靠点 ${at} · 上次：${none(last)}` }),
  chain: (at: number): Words => ({ en: `Panel at stop ${at}`, zh: `面板在停靠点 ${at}` }),
  sheet: (open: boolean, at: number, closed: number): Words => ({
    en: `Sheet ${yes(open).en} · stop ${at} · closed ${closed} times`,
    zh: `面板${yes(open).zh} · 停靠点 ${at} · 已关闭 ${closed} 次`,
  }),
  rows: (rows: number, last: string): Words => ({ en: `${rows} rows · last: ${last}`, zh: `${rows} 行 · 上次：${none(last)}` }),
  pages: (page: number): Words => ({ en: `Page ${page + 1}`, zh: `第 ${page + 1} 页` }),
  nested: (opened: number, wifi: boolean, removed: number): Words => ({
    en: `Row pressed ${opened} times · switch ${wifi ? 'on' : 'off'} · removed ${removed} times`,
    zh: `行已按 ${opened} 次 · 开关${wifi ? '开' : '关'} · 已移除 ${removed} 次`,
  }),
  partly: (checked: number, total: number): Words => ({ en: `${checked} of ${total} checked`, zh: `已选 ${checked} / ${total}` }),
  taps: (n: number): Words => ({ en: `Tapped ${n} times`, zh: `已点 ${n} 次` }),
  beside: (which: 'none' | 'start' | 'end' | 'wide'): Words => {
    const at = { start: labels.sideStart, end: labels.sideEnd, wide: labels.sideWide };
    return which === 'none' ? { en: 'Note closed', zh: '提示已关闭' } : { en: `Note open at: ${at[which].en}`, zh: `提示打开于：${at[which].zh}` };
  },
  menu: (asked: string, last: string): Words => ({
    en: `${asked === 'none' ? 'Menu not asked for' : `Menu asked by ${asked}`} · last: ${last}`,
    zh: `${asked === 'none' ? '尚未唤出菜单' : `菜单由 ${asked} 唤出`} · 上次：${none(last)}`,
  }),
  submenu: (last: string): Words => (last === 'none' ? { en: 'Nothing chosen yet', zh: '尚未选择' } : { en: `Chosen: ${last}`, zh: `已选择：${last}` }),
  panel: (open: boolean): Words => (open ? { en: 'Open', zh: '已展开' } : { en: 'Closed', zh: '已收起' }),
  days: (month: string, day: number): Words => ({ en: `${month} · day ${day} chosen`, zh: `${month} · 已选 ${day} 日` }),
  presses: (n: number): Words => ({ en: `Link pressed ${n} times`, zh: `链接已点 ${n} 次` }),
  suggestions: (open: boolean, active: number, last: string): Words => ({
    en: `List ${yes(open).en} · option ${active + 1} active · last: ${last}`,
    zh: `列表${yes(open).zh} · 第 ${active + 1} 项高亮 · 上次：${none(last)}`,
  }),
} as const;

/** Labels inside the demos that no spec matches by their text. */
export const labels = {
  backToStart: { en: 'Back to start', zh: '回到起点' },
  pullUp: { en: 'Pull up, then keep going', zh: '向上拉，然后继续' },
  openSheet: { en: 'Open sheet', zh: '打开面板' },
  next: { en: 'Next', zh: '下一页' },
  allToppings: { en: 'All toppings', zh: '全部配料' },
  toppings: { en: 'Cheese|Mushroom|Olives', zh: '芝士|蘑菇|橄榄' },
  linkOffLead: { en: 'You can read the ', zh: '你可以阅读' },
  linkOffOld: { en: 'old terms', zh: '旧条款' },
  linkOffMid: { en: ' (withdrawn) or the ', zh: '（已撤回）或' },
  linkOffNew: { en: 'current terms', zh: '现行条款' },
  sideStart: { en: 'At the start', zh: '靠起始侧' },
  sideEnd: { en: 'At the end', zh: '靠末尾侧' },
  sideWide: { en: 'A wide button', zh: '一个很宽的按钮' },
  sideNote: { en: 'A note beside its button.', zh: '按钮旁边的提示。' },
  close: { en: 'Close', zh: '关闭' },
  menuRow: { en: 'Quarterly report.pdf', zh: '季度报告.pdf' },
  menuCopy: { en: 'Copy', zh: '复制' },
  menuRename: { en: 'Rename', zh: '重命名' },
  menuDelete: { en: 'Delete', zh: '删除' },
  rowMenu: { en: 'File actions', zh: '文件操作' },
  months: { en: 'March|April|May', zh: '三月|四月|五月' },
  daysName: { en: 'Days', zh: '日期' },
  subOpen: { en: 'File actions', zh: '文件操作' },
  subRename: { en: 'Rename', zh: '重命名' },
  subMove: { en: 'Move to', zh: '移动到' },
  subInbox: { en: 'Inbox', zh: '收件箱' },
  subArchive: { en: 'Archive', zh: '归档' },
  panelHeading: { en: 'Delivery details', zh: '配送详情' },
  panelBody: { en: 'Leaves the warehouse on Tuesday and arrives within three working days. Someone must be home to sign for it.', zh: '周二从仓库发出，三个工作日内送达，需要有人在家签收。' },
  panelAfter: { en: 'Order total', zh: '订单合计' },
  fruit: { en: 'Fruit', zh: '水果（英文）' },
  definitions: { en: 'Definitions', zh: '术语' },
  updated: { en: 'Updated', zh: '更新于' },
} as const;

/** Picks the side of `words` for the display language the page is in. */
export function say(language: string | undefined): (words: Words) => string {
  const zh = resolveDisplayLanguage(language) === 'zh-CN';
  return words => (zh ? words.zh : words.en);
}
