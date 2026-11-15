// The View catalogue's words, in both display languages: its groups, each demo's stable id for specs, its
// title and the one sentence saying what to try, and what each group does not show yet.
import { resolveDisplayLanguage } from '../display-language';

export type Words = { en: string; zh: string };
export type GroupId = 'text' | 'gestures';

export const groups: readonly { id: GroupId; page: string; title: Words; line: Words }[] = [
  { id: 'text', page: 'view-text', title: { en: 'Text & input', zh: '文字与输入' }, line: { en: 'Paragraphs, fields and keys', zh: '段落、输入框与按键' } },
  { id: 'gestures', page: 'view-gestures', title: { en: 'Gestures & pointer', zh: '手势与指针' }, line: { en: 'Press, drag and swipe', zh: '按压、拖动与滑动' } },
];

export const demos = {
  text: {
    paragraphs: { testId: 'lab-text', title: { en: 'Paragraphs', zh: '段落' }, try: { en: 'Numbers line up in columns, line breaks are kept where asked, and a long address breaks anywhere.', zh: '数字按列对齐，换行按要求保留，长网址可在任意处断开。' } },
    runs: { testId: 'lab-rich-text', title: { en: 'Mixed runs and links', zh: '混排与链接' }, try: { en: 'Tap the link inside the sentence: the count goes up.', zh: '点按句中的链接，计数加一。' } },
    document: { testId: 'lab-document', title: { en: 'A long document', zh: '长文档' }, try: { en: 'Headings, clamped previews and a two-column list of terms from one set of data.', zh: '同一份数据排出标题、截断的摘要和两列的术语表。' } },
    selecting: { testId: 'lab-selection', title: { en: 'Selecting text', zh: '选择文字' }, try: { en: 'Long-press or double-click a word, then drag to extend and copy.', zh: '长按或双击一个词，再拖动扩展选区并复制。' } },
    suggestions: { testId: 'lab-type', title: { en: 'Suggestions while typing', zh: '输入时的建议' }, try: { en: 'Type to filter. Arrows move, Enter chooses, Escape closes; the list opens against the field.', zh: '输入以筛选。方向键移动，回车选择，Esc 关闭；列表贴着输入框打开。' } },
    form: { testId: 'lab-form', title: { en: 'Required, invalid and described fields', zh: '必填、无效与带说明的字段' }, try: { en: 'Each field tells a screen reader its hint and its error.', zh: '每个字段都把提示和错误告诉读屏软件。' } },
  },
  gestures: {
    pointer: { testId: 'lab-scrub', title: { en: 'Where the pointer is', zh: '指针的位置' }, try: { en: 'Drag across the box to move the marker; drag up or down and the page scrolls.', zh: '在方块上横向拖动以移动标记；上下拖动则滚动页面。' } },
    stops: { testId: 'lab-drag', title: { en: 'Drag between stops', zh: '在停靠点之间拖动' }, try: { en: 'Drag the card and let go: it settles on the nearest stop.', zh: '拖动卡片后松手，它停在最近的停靠点。' } },
    chain: { testId: 'lab-chain', title: { en: 'Drag inside a scroll', zh: '滚动区里的拖动' }, try: { en: 'Pull the panel up: it opens first, then its list scrolls.', zh: '向上拉面板：先展开，随后列表才滚动。' } },
    sheet: { testId: 'lab-detents', title: { en: 'A sheet with stops', zh: '带停靠点的面板' }, try: { en: 'Open the sheet, pull it to full height, flick it down to close.', zh: '打开面板，拉到全高，向下轻扫关闭。' } },
    rows: { testId: 'lab-swipe', title: { en: 'Swipe a row for actions', zh: '滑动行显示操作' }, try: { en: 'Swipe a row left for Archive and Delete; a full swipe deletes.', zh: '向左滑动一行显示归档和删除；滑到底即删除。' } },
    pages: { testId: 'lab-pager', title: { en: 'Pages you swipe between', zh: '滑动翻页' }, try: { en: 'Swipe: one page at a time; the buttons do the same.', zh: '滑动一次翻一页；按钮效果相同。' } },
    nested: { testId: 'lab-nested', title: { en: 'A control inside a row', zh: '行里的控件' }, try: { en: 'Tap the switch: only the switch changes. Tap the row: the row is pressed.', zh: '点开关只切换开关；点行则按下整行。' } },
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
  gestures: [
    { en: 'Pull to refresh', zh: '下拉刷新' },
    { en: 'Back from the screen edge', zh: '屏幕边缘返回' },
    { en: 'Slider press', zh: '滑块的按下状态' },
  ],
};

export const chrome = {
  index: { en: 'What a page is built from.', zh: '页面由这些构成。' },
  missing: { en: 'Not shown yet', zh: '尚未展示' },
  demos: { en: 'demos', zh: '项' },
} as const;

/** Picks the side of `words` for the display language the page is in. */
export function say(language: string | undefined): (words: Words) => string {
  const zh = resolveDisplayLanguage(language) === 'zh-CN';
  return words => (zh ? words.zh : words.en);
}
