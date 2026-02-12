import type { LxPickerEventDetail } from './picker.js';

const ITEM_H = 44;
const VISIBLE = 5;
const COL_H = ITEM_H * VISIBLE;
const PAD = Math.floor(VISIBLE / 2) * ITEM_H;

const _cn = typeof navigator !== 'undefined' && /^zh/i.test(navigator.language);
const FIRST_DOW = _cn ? 1 : 0; // 0=Sun 1=Mon

const L = _cn
  ? { cancel: '取消', confirm: '确定', last7: '近7天', last30: '近30天', thisWeek: '本周', lastWeek: '上周', thisMonth: '本月', lastMonth: '上月' }
  : { cancel: 'Cancel', confirm: 'OK', last7: 'Last 7 days', last30: 'Last 30 days', thisWeek: 'This week', lastWeek: 'Last week', thisMonth: 'This month', lastMonth: 'Last month' };

let _css = false;
function ensureCSS() {
  if (_css) return;
  _css = true;
  const s = document.createElement('style');
  s.textContent = `
.lxp-ov{position:fixed;inset:0;z-index:99999}
.lxp-sh{position:fixed;width:320px;background:#fff;border-radius:12px;box-shadow:0 4px 32px rgba(0,0,0,.12),0 0 0 .5px rgba(0,0,0,.06);opacity:0;transform:translateY(-6px);transition:opacity .2s ease,transform .2s ease;-webkit-user-select:none;user-select:none;max-height:70vh;overflow-y:auto;overflow-x:hidden}
.lxp-ov.lxp-in .lxp-sh{opacity:1;transform:translateY(0)}
.lxp-cols{display:flex;position:relative;height:${COL_H}px;margin:16px 0 0}
.lxp-col{flex:1;height:${COL_H}px;overflow-y:auto;padding:${PAD}px 0;overscroll-behavior:contain;scrollbar-width:none;-webkit-overflow-scrolling:touch}
.lxp-col::-webkit-scrollbar{display:none}
.lxp-item{height:${ITEM_H}px;display:flex;align-items:center;justify-content:center;font-size:18px;color:#333;cursor:default;white-space:nowrap;padding:0 8px;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
.lxp-ln-t,.lxp-ln-b{position:absolute;left:20px;right:20px;height:.5px;background:rgba(0,0,0,.18);pointer-events:none;z-index:5}
.lxp-ln-t{top:${PAD}px}.lxp-ln-b{top:${PAD + ITEM_H}px}
.lxp-mk-t,.lxp-mk-b{position:absolute;left:0;right:0;pointer-events:none;z-index:4}
.lxp-mk-t{top:0;height:${PAD}px;background:linear-gradient(#fff 10%,rgba(255,255,255,.4))}
.lxp-mk-b{bottom:0;height:${PAD}px;background:linear-gradient(to top,#fff 10%,rgba(255,255,255,.4))}
.lxp-tsep{position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);font-size:22px;font-weight:600;color:#8e8e93;pointer-events:none;z-index:5;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
.lxp-btns{display:flex;gap:12px;padding:10px 16px 20px}
.lxp-btn{flex:1;height:46px;border:none;border-radius:12px;font-size:17px;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;cursor:pointer;transition:opacity .15s;outline:none}
.lxp-btn:active{opacity:.7}
.lxp-btn:disabled{cursor:default}
.lxp-btn-c{background:#f2f2f7;color:#000;font-weight:500}
.lxp-btn-k{background:#007aff;color:#fff;font-weight:600}
.lxp-cal{padding:10px 8px 0}
.lxp-qk{display:flex;flex-direction:column;gap:6px;padding:0 4px 8px}
.lxp-qr{display:flex;gap:8px}
.lxp-qb{flex:1;height:28px;border:none;border-radius:14px;font-size:13px;font-weight:500;color:#007aff;background:rgba(0,122,255,.08);cursor:pointer;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;outline:none}
.lxp-qb:active{opacity:.7}
.lxp-nav{display:flex;align-items:center;gap:4px;padding:0 4px;height:32px}
.lxp-nb{width:32px;height:32px;border:none;background:none;cursor:pointer;font-size:14px;color:#007aff;display:flex;align-items:center;justify-content:center;border-radius:6px;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;outline:none}
.lxp-nb:disabled{color:#d1d1d6;cursor:default}
.lxp-nb:not(:disabled):hover{background:rgba(0,0,0,.04)}
.lxp-nt{flex:1;text-align:center;font-size:17px;font-weight:600;color:#000;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
.lxp-wk{display:grid;grid-template-columns:repeat(7,1fr);padding:12px 0 4px}
.lxp-wd{text-align:center;font-size:11px;font-weight:500;color:#8e8e93;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
.lxp-gr{display:grid;grid-template-columns:repeat(7,1fr)}
.lxp-dy{height:34px;display:flex;align-items:center;justify-content:center;position:relative;cursor:pointer;font-size:15px;font-weight:500;color:#000;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;background:transparent}
.lxp-dy.ot{color:#c7c7cc}
.lxp-dy.dis{color:#d1d1d6;opacity:.3;cursor:default;pointer-events:none}
.lxp-dy.td::after{content:'';position:absolute;width:30px;height:30px;border-radius:50%;border:1.5px solid #007aff;pointer-events:none;box-sizing:border-box;top:50%;left:50%;transform:translate(-50%,-50%)}
.lxp-dy.sel{color:#fff;z-index:1}
.lxp-dy.sel::before{content:'';position:absolute;width:30px;height:30px;border-radius:50%;background:#007aff;z-index:-1;top:50%;left:50%;transform:translate(-50%,-50%)}
.lxp-dy.sel.td::after{display:none}
.lxp-dy.inr{background:rgba(0,122,255,.15)}
.lxp-dy.rs{background:linear-gradient(to right,transparent 50%,rgba(0,122,255,.15) 50%)}
.lxp-dy.re{background:linear-gradient(to left,transparent 50%,rgba(0,122,255,.15) 50%)}
.lxp-dy.rs.re{background:transparent}
`;
  document.head.appendChild(s);
}

export function renderWebPicker(
  host: HTMLElement,
  props: Record<string, any>,
  onChange: (d: LxPickerEventDetail) => void,
): () => void {
  ensureCSS();

  let alive = true;
  let getValue: () => LxPickerEventDetail = () => ({});

  const mode = props.mode || 'selector';
  const fields = props.fields || 'day';
  const useCal = mode === 'date' && (fields === 'day' || fields === 'range');

  const ov = mk('div', 'lxp-ov');
  const sh = mk('div', 'lxp-sh');
  ov.appendChild(sh);

  function close() {
    if (!alive) return;
    alive = false;
    document.removeEventListener('keydown', onKey);
    ov.classList.remove('lxp-in');
    const done = () => { try { ov.remove(); } catch {} };
    sh.addEventListener('transitionend', done, { once: true });
    setTimeout(done, 250);
  }
  function cancel() { if (!alive) return; onChange({ cancelled: true }); close(); }
  function confirm() { if (!alive) return; onChange({ ...getValue(), confirmed: true }); close(); }

  const cancelBtn = mkBtn('c', props.cancelText || L.cancel, props.cancelButtonColor, props.cancelTextColor);
  const confirmBtn = mkBtn('k', props.confirmText || L.confirm, props.confirmButtonColor, props.confirmTextColor);
  cancelBtn.addEventListener('click', cancel);
  confirmBtn.addEventListener('click', confirm);
  const btns = mk('div', 'lxp-btns');
  btns.appendChild(cancelBtn);
  btns.appendChild(confirmBtn);

  let initScrolls: (() => void) | null = null;

  if (useCal) {
    const cal = buildCalendar(fields, props,
      (val) => host.dispatchEvent(new CustomEvent('scroll', { detail: val, bubbles: true })),
      (ok) => { confirmBtn.disabled = !ok; confirmBtn.style.opacity = ok ? '1' : '0.45'; },
    );
    sh.appendChild(cal.el);
    getValue = cal.getValue;
    if (fields === 'range' && !props.value) {
      confirmBtn.disabled = true;
      confirmBtn.style.opacity = '0.45';
    }
  } else {
    const content = buildContent(mode, props);
    const colsEl = buildWheelColumns(content,
      () => host.dispatchEvent(new CustomEvent('scroll', { detail: content.value(), bubbles: true })),
    );
    sh.appendChild(colsEl);
    getValue = content.value;
    initScrolls = () => content.cols.forEach(c => { if (c.el) c.el.scrollTop = (c.idx || 0) * ITEM_H; });
  }

  sh.appendChild(btns);
  document.body.appendChild(ov);

  void ov.offsetHeight;
  if (initScrolls) initScrolls();
  positionPopover(sh, host);
  requestAnimationFrame(() => ov.classList.add('lxp-in'));

  ov.addEventListener('mousedown', e => { if (e.target === ov) cancel(); });
  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Escape') cancel();
    if (e.key === 'Enter') confirm();
  };
  document.addEventListener('keydown', onKey);

  return () => {
    if (alive) { alive = false; document.removeEventListener('keydown', onKey); }
    try { ov.remove(); } catch {}
  };
}

function mkBtn(type: 'c' | 'k', text: string, bgColor?: string, textColor?: string): HTMLButtonElement {
  const b = document.createElement('button');
  b.className = `lxp-btn lxp-btn-${type}`;
  b.textContent = text;
  if (bgColor) b.style.background = bgColor;
  if (textColor) b.style.color = textColor;
  return b;
}

function getAnchorRect(host: HTMLElement): DOMRect {
  const hr = host.getBoundingClientRect();
  if (hr.width > 0 && hr.height > 0) return hr;
  const prev = host.previousElementSibling as HTMLElement | null;
  if (prev) return prev.getBoundingClientRect();
  if (host.parentElement) return host.parentElement.getBoundingClientRect();
  return new DOMRect(window.innerWidth / 2 - 160, window.innerHeight / 3, 0, 0);
}

function positionPopover(sh: HTMLElement, host: HTMLElement) {
  const W = 320;
  const GAP = 8;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const anchor = getAnchorRect(host);

  let left = anchor.left + anchor.width / 2 - W / 2;
  if (left + W > vw - GAP) left = vw - GAP - W;
  if (left < GAP) left = GAP;

  const shH = sh.offsetHeight;
  const spaceBelow = vh - anchor.bottom - GAP;
  const spaceAbove = anchor.top - GAP;

  let top: number;
  if (shH <= spaceBelow || spaceBelow >= spaceAbove) {
    top = anchor.bottom + GAP;
    sh.style.transformOrigin = 'top left';
  } else {
    top = anchor.top - GAP - shH;
    sh.style.transformOrigin = 'bottom left';
  }

  sh.style.left = left + 'px';
  sh.style.top = top + 'px';
}

function buildWheelColumns(content: Content, onScrollFn: () => void): HTMLElement {
  const wrap = mk('div', 'lxp-cols');
  wrap.appendChild(mk('div', 'lxp-ln-t'));
  wrap.appendChild(mk('div', 'lxp-ln-b'));
  wrap.appendChild(mk('div', 'lxp-mk-t'));
  wrap.appendChild(mk('div', 'lxp-mk-b'));

  if (content.timeSep) {
    const sep = mk('div', 'lxp-tsep');
    sep.textContent = ':';
    wrap.appendChild(sep);
  }

  content.cols.forEach((c, ci) => {
    const colEl = makeColumn(c.items);
    wrap.appendChild(colEl);
    c.el = colEl;
    const cb = () => { content.onScroll?.(ci, idxOf(colEl)); onScrollFn(); };
    attachWheel(colEl, cb);
    attachSnap(colEl, cb);
    attachLiveScroll(colEl, cb);
  });
  return wrap;
}

type DT = { y: number; m: number; d: number };

interface CalendarResult { el: HTMLElement; getValue: () => LxPickerEventDetail }

function buildCalendar(
  fields: string,
  props: Record<string, any>,
  onScroll: (val: LxPickerEventDetail) => void,
  onComplete: (ok: boolean) => void,
): CalendarResult {
  const isRange = fields === 'range';
  const minDT = parseDT(props.start);
  const maxDT = parseDT(props.end);

  const now = new Date();
  let curY = now.getFullYear(), curM = now.getMonth() + 1;
  let selDate: DT | null = null;
  let rStart: DT | null = null;
  let rEnd: DT | null = null;
  let pickingStart = true;
  let tempStart: DT | null = null;
  let complete = !isRange;

  if (isRange) {
    const vals = Array.isArray(props.value) ? props.value : [];
    if (vals[0]) { rStart = parseDT(vals[0]); if (rStart) { curY = rStart.y; curM = rStart.m; } }
    if (vals[1]) rEnd = parseDT(vals[1]);
    complete = !!(rStart && rEnd);
  } else {
    const v = parseDT(props.value);
    if (v) { selDate = v; curY = v.y; curM = v.m; }
    else selDate = { y: now.getFullYear(), m: now.getMonth() + 1, d: now.getDate() };
  }

  const el = mk('div', 'lxp-cal');

  if (isRange) {
    const qk = mk('div', 'lxp-qk');
    const row1: [string, string][] = [[L.last7, 'l7'], [L.last30, 'l30'], [L.thisWeek, 'tw']];
    const row2: [string, string][] = [[L.lastWeek, 'lw'], [L.thisMonth, 'tm'], [L.lastMonth, 'lm']];
    [row1, row2].forEach(row => {
      const r = mk('div', 'lxp-qr');
      row.forEach(([label, key]) => {
        const b = document.createElement('button');
        b.className = 'lxp-qb';
        b.textContent = label;
        b.addEventListener('click', () => applyQuick(key));
        r.appendChild(b);
      });
      qk.appendChild(r);
    });
    el.appendChild(qk);
  }

  const nav = mk('div', 'lxp-nav');
  const btnPY = mkNavBtn('\u00AB');
  const btnPM = mkNavBtn('\u2039');
  const titleEl = mk('span', 'lxp-nt');
  const btnNM = mkNavBtn('\u203A');
  const btnNY = mkNavBtn('\u00BB');
  btnPY.addEventListener('click', () => changeMonth(-12));
  btnPM.addEventListener('click', () => changeMonth(-1));
  btnNM.addEventListener('click', () => changeMonth(1));
  btnNY.addEventListener('click', () => changeMonth(12));
  [btnPY, btnPM, titleEl, btnNM, btnNY].forEach(n => nav.appendChild(n));
  el.appendChild(nav);

  const wk = mk('div', 'lxp-wk');
  weekdayLabels().forEach(d => { const s = mk('span', 'lxp-wd'); s.textContent = d; wk.appendChild(s); });
  el.appendChild(wk);

  const gridEl = mk('div', 'lxp-gr');
  const cells: HTMLDivElement[] = [];
  let currentDays: DT[] = [];
  for (let i = 0; i < 42; i++) {
    const cell = document.createElement('div') as HTMLDivElement;
    cell.className = 'lxp-dy';
    cell.addEventListener('click', () => {
      const day = currentDays[i];
      if (day) selectDay(day);
    });
    cells.push(cell);
    gridEl.appendChild(cell);
  }
  el.appendChild(gridEl);

  render();
  onComplete(complete);

  function changeMonth(offset: number) {
    const d = new Date(curY, curM - 1 + offset, 1);
    curY = d.getFullYear(); curM = d.getMonth() + 1;
    render();
  }

  function applyQuick(key: string) {
    const td = todayDT();
    let s: DT, e: DT;
    switch (key) {
      case 'l7':  s = addDays(td, -6); e = td; break;
      case 'l30': s = addDays(td, -29); e = td; break;
      case 'tw':  s = weekStart(td); e = td; break;
      case 'lw': {
        const ws = weekStart(td);
        s = addDays(ws, -7);
        e = addDays(s, 6);
        break;
      }
      case 'tm':  s = { y: td.y, m: td.m, d: 1 }; e = td; break;
      case 'lm': {
        const som = { y: td.y, m: td.m, d: 1 };
        e = addDays(som, -1);
        s = { y: e.y, m: e.m, d: 1 };
        break;
      }
      default: return;
    }
    rStart = s; rEnd = e;
    pickingStart = true; tempStart = null;
    complete = true; onComplete(true);
    curY = s.y; curM = s.m;
    render();
    emitScroll();
  }

  function selectDay(day: DT) {
    if (minDT && dtKey(day) < dtKey(minDT)) return;
    if (maxDT && dtKey(day) > dtKey(maxDT)) return;

    if (isRange) {
      if (pickingStart) {
        tempStart = day;
        rStart = day; rEnd = day;
        pickingStart = false;
        complete = false; onComplete(false);
      } else {
        const s = tempStart!;
        if (dtKey(day) < dtKey(s)) { rStart = day; rEnd = s; }
        else { rStart = s; rEnd = day; }
        pickingStart = true; tempStart = null;
        complete = true; onComplete(true);
      }
    } else {
      selDate = day;
    }

    if (day.m !== curM || day.y !== curY) { curY = day.y; curM = day.m; }
    render();
    emitScroll();
  }

  function emitScroll() {
    onScroll(getVal());
  }

  function getVal(): LxPickerEventDetail {
    if (isRange && rStart && rEnd) return { value: [fmtDT(rStart), fmtDT(rEnd)] };
    if (selDate) return { value: fmtDT(selDate) };
    return {};
  }

  function render() {
    titleEl.textContent = new Date(curY, curM - 1).toLocaleDateString(navigator.language, { year: 'numeric', month: 'long' });
    updateNavBtns();
    currentDays = calGrid(curY, curM);
    const td = todayDT();
    const sameDay = rStart && rEnd ? dtEq(rStart, rEnd) : false;
    for (let i = 0; i < 42; i++) {
      const cell = cells[i];
      const day = currentDays[i];
      cell.textContent = String(day.d);

      const inMonth = day.m === curM && day.y === curY;
      const disabled = (minDT && dtKey(day) < dtKey(minDT)) || (maxDT && dtKey(day) > dtKey(maxDT));
      const isToday = dtEq(day, td);
      const isStart = rStart ? dtEq(day, rStart) : false;
      const isEnd = rEnd ? dtEq(day, rEnd) : false;
      const isSel = isRange ? (isStart || isEnd) : (selDate ? dtEq(day, selDate) : false);
      const inRange = rStart && rEnd && !sameDay && dtKey(day) >= dtKey(rStart) && dtKey(day) <= dtKey(rEnd);

      let cn = 'lxp-dy';
      if (!inMonth) cn += ' ot';
      if (disabled) cn += ' dis';
      if (isToday && !isSel) cn += ' td';
      if (isSel) cn += ' sel';
      if (isStart && inRange) cn += ' rs';
      if (isEnd && inRange) cn += ' re';
      if (inRange && !isStart && !isEnd) cn += ' inr';
      cell.className = cn;

      cell.style.pointerEvents = disabled ? 'none' : '';
    }
  }

  function updateNavBtns() {
    if (minDT) {
      const prevM = prevMonth(curY, curM);
      const prevY = { y: curY - 1, m: curM };
      btnPM.disabled = dtKey({ ...prevM, d: daysIn(prevM.y, prevM.m) }) < dtKey(minDT);
      btnPY.disabled = dtKey({ ...prevY, d: daysIn(prevY.y, prevY.m) }) < dtKey(minDT);
    }
    if (maxDT) {
      const nextM = nextMonth(curY, curM);
      const nextY = { y: curY + 1, m: curM };
      btnNM.disabled = dtKey({ ...nextM, d: 1 }) > dtKey(maxDT);
      btnNY.disabled = dtKey({ ...nextY, d: 1 }) > dtKey(maxDT);
    }
  }

  return { el, getValue: getVal };
}

function mkNavBtn(text: string): HTMLButtonElement {
  const b = document.createElement('button');
  b.className = 'lxp-nb';
  b.textContent = text;
  return b;
}

interface Col { items: string[]; idx: number; el?: HTMLDivElement }

interface Content {
  cols: Col[];
  timeSep?: boolean;
  value: () => LxPickerEventDetail;
  onScroll?: (colIdx: number, idx: number) => void;
}

function buildContent(mode: string, p: Record<string, any>): Content {
  switch (mode) {
    case 'date': {
      const f = p.fields || 'day';
      if (f === 'year') return yearPicker(p);
      if (f === 'month') return monthPicker(p);
      return yearPicker(p);
    }
    case 'time': return timePicker(p);
    case 'cascading': return cascadePicker(p);
    case 'multiSelector': return multiPicker(p);
    default: return selectorPicker(p);
  }
}

function selectorPicker(p: Record<string, any>): Content {
  const items: string[] = p.columns?.[0] || [];
  const idx = typeof p.defaultIndex === 'number' ? p.defaultIndex : 0;
  const cols: Col[] = [{ items, idx }];
  return {
    cols,
    value: () => { const i = idxOf(cols[0].el!); return { index: i, value: items[i] }; },
  };
}

function multiPicker(p: Record<string, any>): Content {
  const all: string[][] = p.columns || [];
  const defs: number[] = Array.isArray(p.defaultIndex) ? p.defaultIndex : [];
  const cols: Col[] = all.map((items, i) => ({ items, idx: defs[i] || 0 }));
  return {
    cols,
    value: () => {
      const indices = cols.map(c => idxOf(c.el!));
      const values = cols.map((c, i) => c.items[indices[i]]);
      return { index: indices, value: values };
    },
  };
}

function cascadePicker(p: Record<string, any>): Content {
  const data = p.columns as [string[], Record<string, string[]>] | undefined;
  if (!data || data.length < 2) return { cols: [{ items: [], idx: 0 }], value: () => ({}) };
  const [first, map] = data;
  const defs = Array.isArray(p.defaultIndex) ? p.defaultIndex : [0, 0];
  const cols: Col[] = [
    { items: first, idx: defs[0] || 0 },
    { items: map[first[defs[0] || 0]] || [], idx: defs[1] || 0 },
  ];
  return {
    cols,
    value: () => {
      const i0 = idxOf(cols[0].el!), i1 = idxOf(cols[1].el!);
      return { index: [i0, i1], value: [first[i0], (map[first[i0]] || [])[i1]] };
    },
    onScroll: (ci, si) => { if (ci === 0) replaceItems(cols[1].el!, map[first[si]] || []); },
  };
}

function yearPicker(p: Record<string, any>): Content {
  const now = new Date();
  let y = now.getFullYear();
  if (p.value) { const v = parseInt(String(p.value)); if (!isNaN(v)) y = v; }
  const s = p.start ? parseInt(String(p.start).slice(0, 4)) : now.getFullYear() - 10;
  const e = p.end ? parseInt(String(p.end).slice(0, 4)) : now.getFullYear() + 10;
  const years = seq(s, e).map(v => `${v}`);
  const cols: Col[] = [{ items: years, idx: clamp(y - s, 0, years.length - 1) }];
  return {
    cols,
    value: () => { const vy = s + idxOf(cols[0].el!); return { value: `${vy}` }; },
  };
}

function monthPicker(p: Record<string, any>): Content {
  const now = new Date();
  let y = now.getFullYear(), m = now.getMonth() + 1;
  if (p.value) {
    const ps = String(p.value).split('-').map(Number);
    if (ps[0]) y = ps[0]; if (ps[1]) m = ps[1];
  }
  const sY = p.start ? parseInt(String(p.start).slice(0, 4)) : now.getFullYear() - 10;
  const eY = p.end ? parseInt(String(p.end).slice(0, 4)) : now.getFullYear() + 10;
  const years = seq(sY, eY).map(v => `${v}`);
  const months = seq(1, 12).map(v => _cn ? `${v}月` : String(v).padStart(2, '0'));
  const cols: Col[] = [
    { items: years, idx: clamp(y - sY, 0, years.length - 1) },
    { items: months, idx: clamp(m - 1, 0, 11) },
  ];
  return {
    cols,
    value: () => {
      const vy = sY + idxOf(cols[0].el!);
      const vm = idxOf(cols[1].el!) + 1;
      return { value: `${vy}-${z(vm)}` };
    },
  };
}

function timePicker(p: Record<string, any>): Content {
  const startMin = parseTime(p.start) ?? 0;
  const endMin = parseTime(p.end) ?? 23 * 60 + 59;
  const startH = Math.floor(startMin / 60), endH = Math.floor(endMin / 60);
  const sM = startMin % 60, eM = endMin % 60;

  function minsFor(h: number): number[] {
    if (startH === endH) return seq(sM, eM);
    if (h === startH) return seq(sM, 59);
    if (h === endH) return seq(0, eM);
    return seq(0, 59);
  }

  let curH = startH, curMinute = 0;
  if (p.value) {
    const init = parseTime(p.value) ?? startMin;
    const clamped = clamp(init, startMin, endMin);
    curH = Math.floor(clamped / 60);
    curMinute = clamped % 60;
  }

  const hours = seq(startH, endH);
  const initMins = minsFor(curH);
  const cols: Col[] = [
    { items: hours.map(v => z(v)), idx: curH - startH },
    { items: initMins.map(v => z(v)), idx: Math.max(0, initMins.indexOf(curMinute)) },
  ];

  return {
    cols,
    timeSep: true,
    value: () => {
      const vh = startH + idxOf(cols[0].el!);
      const ma = minsFor(vh);
      const mi = clamp(idxOf(cols[1].el!), 0, ma.length - 1);
      return { value: `${z(vh)}:${z(ma[mi])}` };
    },
    onScroll: (ci) => {
      if (ci === 0) {
        const vh = startH + idxOf(cols[0].el!);
        const newMins = minsFor(vh);
        replaceItems(cols[1].el!, newMins.map(v => z(v)));
        cols[1].items = newMins.map(v => z(v));
      }
    },
  };
}

function mk(tag: string, cls: string) { const e = document.createElement(tag); e.className = cls; return e; }

function makeColumn(items: string[]): HTMLDivElement {
  const col = document.createElement('div');
  col.className = 'lxp-col';
  items.forEach((text, i) => {
    const d = mk('div', 'lxp-item');
    d.textContent = text;
    d.addEventListener('click', () => col.scrollTo({ top: i * ITEM_H, behavior: 'smooth' }));
    col.appendChild(d);
  });
  return col;
}

function idxOf(col: HTMLElement): number { return Math.max(0, Math.round(col.scrollTop / ITEM_H)); }

function attachWheel(col: HTMLElement, onMove?: () => void) {
  let acc = 0;
  col.addEventListener('wheel', e => {
    e.preventDefault();
    acc += e.deltaY;
    const threshold = Math.abs(e.deltaY) > 20 ? 1 : ITEM_H / 2;
    if (Math.abs(acc) >= threshold) {
      const dir = Math.sign(acc);
      acc = 0;
      const maxIdx = Math.max(0, Math.round((col.scrollHeight - col.clientHeight) / ITEM_H));
      const cur = idxOf(col);
      const next = clamp(cur + dir, 0, maxIdx);
      if (next !== cur) { col.scrollTo({ top: next * ITEM_H, behavior: 'smooth' }); onMove?.(); }
    }
  }, { passive: false });
}

function attachSnap(col: HTMLElement, onSettled?: () => void) {
  let t: number;
  let snapping = false;
  col.addEventListener('scroll', () => {
    if (snapping) return;
    clearTimeout(t);
    t = window.setTimeout(() => {
      const idx = idxOf(col);
      const target = idx * ITEM_H;
      if (Math.abs(col.scrollTop - target) > 1) {
        snapping = true;
        col.scrollTo({ top: target, behavior: 'smooth' });
        setTimeout(() => { snapping = false; }, 300);
      }
      onSettled?.();
    }, 120);
  });
}

function attachLiveScroll(col: HTMLElement, onMove?: () => void) {
  let last = -1;
  col.addEventListener('scroll', () => {
    const cur = idxOf(col);
    if (cur !== last) { last = cur; onMove?.(); }
  });
}

function replaceItems(col: HTMLDivElement, items: string[]) {
  const frag = document.createDocumentFragment();
  items.forEach((text, i) => {
    const d = mk('div', 'lxp-item');
    d.textContent = text;
    d.addEventListener('click', () => col.scrollTo({ top: i * ITEM_H, behavior: 'smooth' }));
    frag.appendChild(d);
  });
  col.innerHTML = '';
  col.appendChild(frag);
  col.scrollTop = 0;
}

function dtKey(d: DT) { return d.y * 10000 + d.m * 100 + d.d; }
function dtEq(a: DT, b: DT) { return dtKey(a) === dtKey(b); }
function todayDT(): DT { const n = new Date(); return { y: n.getFullYear(), m: n.getMonth() + 1, d: n.getDate() }; }

function addDays(d: DT, days: number): DT {
  const date = new Date(d.y, d.m - 1, d.d + days);
  return { y: date.getFullYear(), m: date.getMonth() + 1, d: date.getDate() };
}

function weekStart(d: DT): DT {
  const date = new Date(d.y, d.m - 1, d.d);
  const diff = (date.getDay() - FIRST_DOW + 7) % 7;
  return addDays(d, -diff);
}

function prevMonth(y: number, m: number) { const d = new Date(y, m - 2, 1); return { y: d.getFullYear(), m: d.getMonth() + 1 }; }
function nextMonth(y: number, m: number) { const d = new Date(y, m, 1); return { y: d.getFullYear(), m: d.getMonth() + 1 }; }

function calGrid(y: number, m: number): DT[] {
  const first = new Date(y, m - 1, 1);
  const offset = (first.getDay() - FIRST_DOW + 7) % 7;
  return Array.from({ length: 42 }, (_, i) => {
    const d = new Date(y, m - 1, 1 - offset + i);
    return { y: d.getFullYear(), m: d.getMonth() + 1, d: d.getDate() };
  });
}

function weekdayLabels(): string[] {
  return Array.from({ length: 7 }, (_, i) => {
    const d = new Date(2025, 0, 5 + ((i + FIRST_DOW) % 7));
    return d.toLocaleDateString(navigator.language, { weekday: 'short' });
  });
}

function parseDT(s?: string | null): DT | null {
  if (!s || typeof s !== 'string') return null;
  const p = s.split('-').map(Number);
  if (!p[0] || isNaN(p[0])) return null;
  return { y: p[0], m: p[1] || 1, d: p[2] || 1 };
}
function fmtDT(d: DT) { return `${d.y}-${z(d.m)}-${z(d.d)}`; }
function daysIn(y: number, m: number) { return new Date(y, m, 0).getDate(); }

function seq(a: number, b: number): number[] { const r: number[] = []; for (let i = a; i <= b; i++) r.push(i); return r; }
function z(n: number) { return n < 10 ? `0${n}` : `${n}`; }
function clamp(v: number, lo: number, hi: number) { return Math.max(lo, Math.min(hi, v)); }

function parseTime(s?: string): number | null {
  if (!s) return null;
  const p = String(s).split(':').map(Number);
  if (p.length < 2 || isNaN(p[0]) || isNaN(p[1])) return null;
  return clamp(p[0], 0, 23) * 60 + clamp(p[1], 0, 59);
}
