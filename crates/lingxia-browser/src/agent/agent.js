// LingXia browser agent: page snapshot, element refs, actionability, form
// input, readable text and set-of-marks overlays for agent-driven browsing.
//
// Installed lazily into the top document by the host (`agent/mod.rs`) and
// called through `window.__LingXiaAgent.call(op, args)`. One instance per
// document: a navigation replaces the document, and with it every ref, so
// `generation` tells the host which document issued a ref.
//
// Every call returns `{ ok: true, ... }` or `{ ok: false, error: { kind,
// message, ... } }`; the kinds are the typed failures the control protocol
// reports (`not_found`, `stale_ref`, `not_visible`, `disabled`, `covered`,
// `not_editable`, `unsupported`, `usage`).
(() => {
  const VERSION = '__LX_AGENT_VERSION__';
  const existing = window.__LingXiaAgent;
  if (existing && existing.version === VERSION) return;

  const generation =
    Math.random().toString(36).slice(2, 10) + Date.now().toString(36).slice(-4);
  // Refs are unique per tab, not per document: the host passes where the
  // previous document's refs ended, so an old ref can never name an element
  // of this page.
  let nextRef = Math.max(1, Number(window.__LingXiaAgentRefStart) || 1);
  try { delete window.__LingXiaAgentRefStart; } catch (_e) {}
  const refOf = new WeakMap();
  const byRef = new Map();
  let lastSnapshot = null;
  let marksHost = null;
  let consoleCursor = 0;

  // ---------------------------------------------------------------------
  // Results

  const ok = (fields) =>
    Object.assign({ ok: true, generation, refs_issued: nextRef - 1 }, fields || {});
  const fail = (kind, message, extra) => ({
    ok: false,
    generation,
    error: Object.assign({ kind, message }, extra || {}),
  });

  // ---------------------------------------------------------------------
  // Refs

  function assignRef(el) {
    let ref = refOf.get(el);
    if (!ref) {
      ref = 'e' + nextRef++;
      refOf.set(el, ref);
      byRef.set(ref, new WeakRef(el));
    }
    return ref;
  }

  function resolveTarget(target) {
    if (!target || typeof target !== 'object') {
      return { error: fail('usage', 'target must be { ref } or { selector }') };
    }
    const hasRef = typeof target.ref === 'string' && target.ref !== '';
    const hasSelector = typeof target.selector === 'string' && target.selector.trim() !== '';
    if (hasRef === hasSelector) {
      return { error: fail('usage', 'pass exactly one of ref or selector') };
    }
    if (hasRef) {
      const weak = byRef.get(target.ref);
      if (!weak) {
        return {
          error: fail('stale_ref', `ref ${target.ref} was not issued by this document`, {
            reason: 'unknown',
            ref: target.ref,
          }),
        };
      }
      const el = weak.deref();
      if (!el || !el.isConnected) {
        byRef.delete(target.ref);
        return {
          error: fail('stale_ref', `ref ${target.ref} was removed from the page`, {
            reason: 'removed',
            ref: target.ref,
          }),
        };
      }
      return { el };
    }
    let nodes;
    try {
      nodes = document.querySelectorAll(target.selector);
    } catch (err) {
      return {
        error: fail('usage', 'invalid selector: ' + (err && err.message ? err.message : err)),
      };
    }
    const index = Number.isInteger(target.index) && target.index >= 0 ? target.index : 0;
    if (!nodes.length || index >= nodes.length) {
      return {
        error: fail('not_found', `no element matches ${target.selector}`, {
          selector: target.selector,
          count: nodes.length,
          index,
        }),
      };
    }
    return { el: nodes[index] };
  }

  // ---------------------------------------------------------------------
  // Geometry, visibility, hit testing

  function frameChain(el) {
    // Frame elements from the top document down to the element's document.
    const chain = [];
    let win = el.ownerDocument && el.ownerDocument.defaultView;
    while (win && win !== window) {
      let fe = null;
      try {
        fe = win.frameElement;
      } catch (_e) {
        fe = null;
      }
      if (!fe) return null;
      chain.unshift(fe);
      win = fe.ownerDocument.defaultView;
    }
    return chain;
  }

  function frameContentOffset(fe) {
    const r = fe.getBoundingClientRect();
    const style = fe.ownerDocument.defaultView.getComputedStyle(fe);
    return {
      x: r.left + fe.clientLeft + (parseFloat(style.paddingLeft) || 0),
      y: r.top + fe.clientTop + (parseFloat(style.paddingTop) || 0),
    };
  }

  // Rect in top-window viewport coordinates (CSS px).
  function topRect(el) {
    const r = el.getBoundingClientRect();
    let x = r.left;
    let y = r.top;
    const chain = frameChain(el);
    if (chain) {
      for (const fe of chain) {
        const o = frameContentOffset(fe);
        x += o.x;
        y += o.y;
      }
    }
    return { x, y, width: r.width, height: r.height };
  }

  function styleOf(el) {
    return el.ownerDocument.defaultView.getComputedStyle(el);
  }

  function isRendered(el) {
    if (!el.isConnected) return false;
    if (typeof el.checkVisibility === 'function') {
      try {
        if (!el.checkVisibility({ opacityProperty: true, visibilityProperty: true })) return false;
      } catch (_e) {}
    } else {
      for (let n = el; n && n.nodeType === 1; n = n.parentElement) {
        const s = styleOf(n);
        if (s.display === 'none' || Number(s.opacity) === 0) return false;
      }
      if (styleOf(el).visibility !== 'visible') return false;
    }
    return true;
  }

  function isVisible(el) {
    if (!isRendered(el)) return false;
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0;
  }

  function inViewport(rect) {
    return (
      rect.x + rect.width > 0 &&
      rect.y + rect.height > 0 &&
      rect.x < window.innerWidth &&
      rect.y < window.innerHeight
    );
  }

  function fullyInViewport(rect) {
    return (
      rect.x >= 0 &&
      rect.y >= 0 &&
      rect.x + rect.width <= window.innerWidth &&
      rect.y + rect.height <= window.innerHeight
    );
  }

  function deepElementFromPoint(root, x, y) {
    let hit = root.elementFromPoint(x, y);
    while (hit && hit.shadowRoot) {
      const inner = hit.shadowRoot.elementFromPoint(x, y);
      if (!inner || inner === hit) break;
      hit = inner;
    }
    return hit;
  }

  function composedContains(ancestor, node) {
    while (node) {
      if (node === ancestor) return true;
      if (node.assignedSlot && ancestor.contains && ancestor.contains(node.assignedSlot)) {
        return true;
      }
      node = node.parentNode || (node instanceof ShadowRoot ? node.host : null);
      if (node && node.nodeType === 11 && node.host) node = node.host;
    }
    return false;
  }

  // Which element would receive a pointer event at the element's center:
  // null when the target itself (or a descendant) does, otherwise the cover.
  function coveringElement(el, point) {
    const chain = frameChain(el) || [];
    const levels = chain.concat([el]);
    let doc = document;
    let x = point.x;
    let y = point.y;
    for (const expected of levels) {
      const hit = deepElementFromPoint(doc, x, y);
      if (!hit) return null;
      if (!composedContains(expected, hit)) return hit;
      if (expected !== el) {
        const o = frameContentOffset(expected);
        x -= o.x;
        y -= o.y;
        doc = expected.contentDocument;
        if (!doc) return null;
      }
    }
    return null;
  }

  function scrollIntoViewIfNeeded(el) {
    const chain = frameChain(el) || [];
    for (const node of chain.concat([el])) {
      const rect = topRect(node);
      if (!fullyInViewport(rect)) {
        try {
          node.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' });
        } catch (_e) {
          node.scrollIntoView();
        }
      }
    }
  }

  // ---------------------------------------------------------------------
  // Roles, names, states

  const INPUT_ROLES = {
    button: 'button',
    submit: 'button',
    reset: 'button',
    image: 'button',
    checkbox: 'checkbox',
    radio: 'radio',
    range: 'slider',
    number: 'spinbutton',
    search: 'searchbox',
    email: 'textbox',
    tel: 'textbox',
    text: 'textbox',
    url: 'textbox',
    password: 'textbox',
    date: 'textbox',
    'datetime-local': 'textbox',
    month: 'textbox',
    time: 'textbox',
    week: 'textbox',
    color: 'textbox',
    file: 'button',
  };

  const INTERACTIVE_ROLES = new Set([
    'button', 'link', 'checkbox', 'radio', 'switch', 'tab', 'menuitem',
    'menuitemcheckbox', 'menuitemradio', 'option', 'combobox', 'textbox',
    'searchbox', 'slider', 'spinbutton', 'treeitem', 'listbox', 'gridcell',
  ]);

  const LANDMARKS = new Set([
    'banner', 'navigation', 'main', 'complementary', 'contentinfo', 'search',
    'form', 'region',
  ]);

  const NAME_FROM_CONTENT = new Set([
    'button', 'link', 'heading', 'tab', 'menuitem', 'menuitemcheckbox',
    'menuitemradio', 'option', 'treeitem', 'cell', 'columnheader', 'rowheader',
    'checkbox', 'radio', 'switch', 'tooltip', 'listitem',
  ]);

  function tagOf(el) {
    return (el.localName || el.tagName || '').toLowerCase();
  }

  function hasSectioningAncestor(el) {
    for (let n = el.parentElement; n; n = n.parentElement) {
      const t = tagOf(n);
      if (t === 'article' || t === 'aside' || t === 'main' || t === 'nav' || t === 'section') {
        return true;
      }
    }
    return false;
  }

  function roleOf(el) {
    const explicit = (el.getAttribute('role') || '').trim().split(/\s+/)[0];
    if (explicit && explicit !== 'presentation' && explicit !== 'none') return explicit;
    if (explicit === 'presentation' || explicit === 'none') return 'none';
    const tag = tagOf(el);
    switch (tag) {
      case 'a':
      case 'area':
        return el.hasAttribute('href') ? 'link' : 'generic';
      case 'button':
        return 'button';
      case 'summary':
        return 'button';
      case 'input': {
        const type = (el.getAttribute('type') || 'text').toLowerCase();
        if (type === 'hidden') return 'none';
        if ((type === 'text' || type === 'search' || type === 'email' || type === 'tel' ||
          type === 'url') && el.hasAttribute('list')) return 'combobox';
        return INPUT_ROLES[type] || 'textbox';
      }
      case 'select':
        return el.multiple || el.size > 1 ? 'listbox' : 'combobox';
      case 'option':
        return 'option';
      case 'textarea':
        return 'textbox';
      case 'h1': case 'h2': case 'h3': case 'h4': case 'h5': case 'h6':
        return 'heading';
      case 'img':
        return el.getAttribute('alt') === '' ? 'none' : 'img';
      case 'svg':
        return 'img';
      case 'nav':
        return 'navigation';
      case 'main':
        return 'main';
      case 'header':
        return hasSectioningAncestor(el) ? 'generic' : 'banner';
      case 'footer':
        return hasSectioningAncestor(el) ? 'generic' : 'contentinfo';
      case 'aside':
        return 'complementary';
      case 'form':
        return 'form';
      case 'search':
        return 'search';
      case 'section':
        // Named sections are regions; checked directly, since naming
        // consults the role.
        return el.hasAttribute('aria-label') || el.hasAttribute('aria-labelledby') ||
          el.hasAttribute('title') ? 'region' : 'generic';
      case 'article':
        return 'article';
      case 'ul':
      case 'ol':
      case 'menu':
        return 'list';
      case 'li':
        return 'listitem';
      case 'dl':
        return 'list';
      case 'dt':
      case 'dd':
        return 'listitem';
      case 'table':
        return 'table';
      case 'tr':
        return 'row';
      case 'th':
        return el.getAttribute('scope') === 'row' ? 'rowheader' : 'columnheader';
      case 'td':
        return 'cell';
      case 'dialog':
        return 'dialog';
      case 'fieldset':
        return 'group';
      case 'details':
        return 'group';
      case 'progress':
        return 'progressbar';
      case 'meter':
        return 'meter';
      case 'p':
        return 'paragraph';
      case 'blockquote':
        return 'blockquote';
      case 'pre':
      case 'code':
        return 'code';
      case 'hr':
        return 'separator';
      case 'iframe':
      case 'frame':
        return 'iframe';
      case 'label':
        return 'label';
      case 'figure':
        return 'figure';
      case 'video':
      case 'audio':
        return tag;
      default:
        if (el.isContentEditable && el.getAttribute('contenteditable') !== null) return 'textbox';
        return 'generic';
    }
  }

  function collapse(text) {
    return String(text || '').replace(/\s+/g, ' ').trim();
  }

  function clip(text, max) {
    const t = collapse(text);
    return t.length > max ? t.slice(0, max - 1) + '…' : t;
  }

  function textFromContent(el, depth) {
    if (depth > 4) return '';
    let out = '';
    const visit = (node) => {
      if (out.length > 400) return;
      if (node.nodeType === 3) {
        out += node.nodeValue;
        return;
      }
      if (node.nodeType !== 1) return;
      const tag = tagOf(node);
      if (tag === 'script' || tag === 'style' || tag === 'noscript' || tag === 'template') return;
      // A label's text names its control; the control's own content does not.
      if (node !== el && (tag === 'select' || tag === 'textarea' || tag === 'datalist')) return;
      if (node !== el && !isRendered(node)) return;
      if (node.getAttribute('aria-hidden') === 'true') return;
      if (tag === 'img' || tag === 'input') {
        const alt = node.getAttribute('alt') || node.getAttribute('aria-label');
        if (alt) out += ' ' + alt + ' ';
        if (tag === 'input' && (node.type === 'submit' || node.type === 'button')) {
          out += ' ' + (node.value || '') + ' ';
        }
        return;
      }
      if (node !== el && node.getAttribute('aria-label')) {
        out += ' ' + node.getAttribute('aria-label') + ' ';
        return;
      }
      const root = node.shadowRoot || node;
      for (const child of root.childNodes) visit(child);
      if (node.shadowRoot) for (const child of node.childNodes) visit(child);
    };
    visit(el);
    return collapse(out);
  }

  function labelsText(el) {
    const parts = [];
    if (el.labels && el.labels.length) {
      for (const label of el.labels) parts.push(textFromContent(label, 1));
    }
    return collapse(parts.join(' '));
  }

  function accessibleName(el, depth) {
    depth = depth || 0;
    const labelledby = el.getAttribute('aria-labelledby');
    if (labelledby && depth === 0) {
      const doc = el.ownerDocument;
      const text = labelledby
        .split(/\s+/)
        .map((id) => doc.getElementById(id))
        .filter(Boolean)
        .map((n) => textFromContent(n, 1))
        .join(' ');
      if (collapse(text)) return collapse(text);
    }
    const aria = el.getAttribute('aria-label');
    if (aria && collapse(aria)) return collapse(aria);
    const tag = tagOf(el);
    if (tag === 'input' || tag === 'textarea' || tag === 'select') {
      const type = (el.getAttribute('type') || '').toLowerCase();
      if (tag === 'input' && (type === 'submit' || type === 'reset' || type === 'button')) {
        return collapse(el.value || (type === 'submit' ? 'Submit' : type === 'reset' ? 'Reset' : ''));
      }
      if (tag === 'input' && type === 'image') return collapse(el.getAttribute('alt') || 'Submit');
      const labels = labelsText(el);
      if (labels) return labels;
      const title = el.getAttribute('title');
      if (title) return collapse(title);
      const placeholder = el.getAttribute('placeholder');
      if (placeholder) return collapse(placeholder);
      return '';
    }
    if (tag === 'img' || tag === 'area') {
      return collapse(el.getAttribute('alt') || el.getAttribute('title') || '');
    }
    if (tag === 'iframe' || tag === 'frame') {
      return collapse(el.getAttribute('title') || el.getAttribute('name') || '');
    }
    if (tag === 'fieldset') {
      const legend = el.querySelector(':scope > legend');
      if (legend) return textFromContent(legend, 1);
    }
    if (tag === 'table') {
      const caption = el.querySelector(':scope > caption');
      if (caption) return textFromContent(caption, 1);
    }
    if (tag === 'svg') {
      const title = el.querySelector(':scope > title');
      if (title) return collapse(title.textContent);
    }
    const role = roleOf(el);
    if (NAME_FROM_CONTENT.has(role) || (tag === 'a' && el.hasAttribute('href'))) {
      const text = textFromContent(el, depth);
      if (text) return text;
    }
    const title = el.getAttribute('title');
    return title ? collapse(title) : '';
  }

  function isDisabled(el) {
    try {
      if (el.matches(':disabled')) return true;
    } catch (_e) {}
    return !!(el.closest && el.closest('[aria-disabled="true"]'));
  }

  const NON_TEXT_INPUTS = new Set([
    'button', 'checkbox', 'color', 'file', 'hidden', 'image', 'radio', 'range', 'reset',
    'submit',
  ]);

  function isTextField(el) {
    const tag = tagOf(el);
    if (tag === 'textarea') return true;
    if (tag === 'input') return !NON_TEXT_INPUTS.has((el.type || 'text').toLowerCase());
    return false;
  }

  // Inputs whose value is set (not typed): date pickers, sliders, colors.
  function isValueField(el) {
    if (tagOf(el) !== 'input') return false;
    const type = (el.type || '').toLowerCase();
    return type === 'range' || type === 'color' || type === 'date' || type === 'time' ||
      type === 'datetime-local' || type === 'month' || type === 'week' || type === 'number';
  }

  function isEditable(el) {
    if (el.isContentEditable) return true;
    if (isTextField(el) || isValueField(el)) return !el.readOnly && !isDisabled(el);
    return false;
  }

  function isInteractive(el, role) {
    if (INTERACTIVE_ROLES.has(role)) return true;
    const tag = tagOf(el);
    if (tag === 'input' || tag === 'select' || tag === 'textarea' || tag === 'summary') return true;
    if (tag === 'iframe' || tag === 'video' || tag === 'audio') return true;
    if (el.isContentEditable && el.getAttribute('contenteditable') !== null) return true;
    const tabindex = el.getAttribute('tabindex');
    if (tabindex !== null && Number(tabindex) >= 0) return true;
    if (el.hasAttribute('onclick') || el.getAttribute('draggable') === 'true') return true;
    // SPA "div buttons": the outermost element that shows a pointer cursor.
    if (styleOf(el).cursor === 'pointer') {
      const parent = el.parentElement;
      if (!parent || styleOf(parent).cursor !== 'pointer') return true;
    }
    return false;
  }

  function currentValue(el) {
    const tag = tagOf(el);
    if (tag === 'select') {
      const selected = Array.from(el.selectedOptions || []).map((o) => collapse(o.label || o.text));
      return selected.join(', ');
    }
    if (tag === 'input') {
      const type = (el.type || 'text').toLowerCase();
      if (type === 'password') return el.value ? '••••' : '';
      if (type === 'checkbox' || type === 'radio' || type === 'file') {
        if (type === 'file') return Array.from(el.files || []).map((f) => f.name).join(', ');
        return null;
      }
      return el.value;
    }
    if (tag === 'textarea') return el.value;
    if (el.isContentEditable && el.getAttribute('contenteditable') !== null) {
      return collapse(el.innerText);
    }
    const valuenow = el.getAttribute('aria-valuenow');
    if (valuenow !== null) return el.getAttribute('aria-valuetext') || valuenow;
    return null;
  }

  function checkedState(el, role) {
    const tag = tagOf(el);
    if (tag === 'input' && (el.type === 'checkbox' || el.type === 'radio')) {
      if (el.indeterminate) return 'mixed';
      return el.checked;
    }
    if (role === 'checkbox' || role === 'radio' || role === 'switch' ||
      role === 'menuitemcheckbox' || role === 'menuitemradio' || role === 'treeitem') {
      const aria = el.getAttribute('aria-checked');
      if (aria === 'mixed') return 'mixed';
      if (aria === 'true') return true;
      if (aria === 'false') return false;
    }
    return undefined;
  }

  function statesOf(el, role) {
    const states = {};
    const checked = checkedState(el, role);
    if (checked !== undefined) states.checked = checked;
    if (tagOf(el) === 'option') {
      if (el.selected) states.selected = true;
    } else if (el.getAttribute('aria-selected') === 'true') {
      states.selected = true;
    }
    const expanded = el.getAttribute('aria-expanded');
    if (expanded === 'true') states.expanded = true;
    else if (expanded === 'false') states.expanded = false;
    if (tagOf(el) === 'details') states.expanded = el.open;
    const pressed = el.getAttribute('aria-pressed');
    if (pressed === 'true') states.pressed = true;
    if (isDisabled(el)) states.disabled = true;
    if (el.required || el.getAttribute('aria-required') === 'true') states.required = true;
    if ((isTextField(el) || isValueField(el)) && el.readOnly) states.readonly = true;
    if (el.getAttribute('aria-invalid') === 'true') states.invalid = true;
    const doc = el.ownerDocument;
    if (doc.activeElement === el && doc.hasFocus && doc.hasFocus()) states.focused = true;
    return states;
  }

  function headingLevel(el) {
    const m = /^h([1-6])$/.exec(tagOf(el));
    if (m) return Number(m[1]);
    const aria = Number(el.getAttribute('aria-level'));
    return aria > 0 ? aria : 2;
  }

  function absoluteUrl(el, attr) {
    const raw = el.getAttribute(attr);
    if (!raw) return null;
    try {
      return new URL(raw, el.ownerDocument.baseURI).href;
    } catch (_e) {
      return raw;
    }
  }

  function describe(el) {
    if (!el || el.nodeType !== 1) return null;
    const tag = tagOf(el);
    let selector = tag;
    if (el.id) selector += '#' + el.id;
    else if (el.classList && el.classList.length) {
      selector += '.' + Array.from(el.classList).slice(0, 2).join('.');
    }
    const role = roleOf(el);
    const name = clip(accessibleName(el), 60);
    const out = { ref: assignRef(el), selector, role };
    if (name) out.name = name;
    const text = clip(el.innerText || el.textContent || '', 60);
    if (text && text !== name) out.text = text;
    return out;
  }

  // ---------------------------------------------------------------------
  // Snapshot

  const SKIP_TAGS = new Set([
    'script', 'style', 'noscript', 'template', 'link', 'meta', 'head', 'title', 'base',
    'datalist',
  ]);

  // Roles that carry structure but not text of their own: they stay in the
  // tree when they have a name or children worth showing.
  const CONTAINER_ROLES = new Set([
    'list', 'listitem', 'table', 'row', 'group', 'article', 'figure', 'dialog',
    'blockquote', 'form', 'region', 'search', 'banner', 'navigation', 'main',
    'complementary', 'contentinfo', 'menu', 'menubar', 'tablist', 'tabpanel', 'toolbar',
    'tree', 'grid', 'rowgroup', 'radiogroup', 'alertdialog', 'alert', 'status', 'log',
  ]);

  function buildTree(rootEl, opts) {
    const maxNodes = opts.maxNodes;
    const budget = { nodes: 0, truncated: false, omitted: 0 };
    let refCount = 0;

    const take = () => {
      if (budget.nodes >= maxNodes) {
        budget.truncated = true;
        budget.omitted++;
        return false;
      }
      budget.nodes++;
      return true;
    };

    function textNode(text) {
      const t = clip(text, opts.maxText);
      return t ? { role: 'text', name: t } : null;
    }

    // Children of a node: element children (through open shadow roots and
    // slots) plus text runs.
    function childrenOf(el) {
      const out = [];
      const pushText = (buf) => {
        const t = collapse(buf);
        if (t) {
          const node = take() ? textNode(t) : null;
          if (node) out.push(node);
        }
      };
      const visitList = (nodes) => {
        let buf = '';
        for (const child of nodes) {
          if (child.nodeType === 3) {
            buf += child.nodeValue;
            continue;
          }
          if (child.nodeType !== 1) continue;
          if (tagOf(child) === 'slot') {
            const assigned = child.assignedNodes({ flatten: true });
            if (buf) { pushText(buf); buf = ''; }
            visitList(assigned.length ? assigned : child.childNodes);
            continue;
          }
          if (buf) { pushText(buf); buf = ''; }
          for (const n of visitElement(child)) out.push(n);
        }
        if (buf) pushText(buf);
      };
      if (el.shadowRoot) visitList(el.shadowRoot.childNodes);
      else visitList(el.childNodes);
      return out;
    }

    function frameNode(el, base) {
      let doc = null;
      try {
        doc = el.contentDocument;
      } catch (_e) {
        doc = null;
      }
      const src = absoluteUrl(el, 'src') || (el.src || null);
      if (src) base.url = src;
      if (!doc) {
        base.frame = 'cross-origin';
        return base;
      }
      base.frame = 'same-origin';
      if (doc.body) base.children = childrenOf(doc.body);
      return base;
    }

    // Returns a list: an element either becomes one node, or is transparent
    // and contributes its children.
    function visitElement(el) {
      const tag = tagOf(el);
      if (SKIP_TAGS.has(tag)) return [];
      if (el.getAttribute('aria-hidden') === 'true') return [];
      if (el.hidden && tag !== 'details') return [];
      const toggle = tag === 'input' && (el.type === 'checkbox' || el.type === 'radio');
      // Styled controls hide the real checkbox (opacity 0, 1px) behind a label.
      const rendered = isRendered(el) || (toggle && styleOf(el).display !== 'none' && styleOf(el).visibility !== 'hidden');
      if (!rendered) {
        // display:contents and zero-size wrappers still hold visible children.
        const s = styleOf(el);
        if (s.display === 'none' || s.visibility === 'hidden') return [];
        if (s.display !== 'contents') return [];
      }
      const role = roleOf(el);
      if (role === 'none' && tag === 'input') return [];
      const interactive = isInteractive(el, role);

      if (tag === 'select') {
        if (!take()) return [];
        const node = { role, name: accessibleName(el), el };
        node.ref = assignRef(el);
        refCount++;
        const value = currentValue(el);
        if (value) node.value = value;
        node.states = statesOf(el, role);
        node.options = Array.from(el.options).slice(0, 50).map((o) => {
          const opt = { label: collapse(o.label || o.text), value: o.value };
          if (o.selected) opt.selected = true;
          if (o.disabled) opt.disabled = true;
          return opt;
        });
        if (el.options.length > 50) node.optionsTruncated = el.options.length - 50;
        return [node];
      }

      if (tag === 'label') {
        // The control already carries the label text as its name.
        if (el.control) return childrenOf(el).filter((c) => c.role !== 'text' && c.role !== 'generic');
        return childrenOf(el);
      }

      const keep =
        interactive ||
        role === 'heading' ||
        role === 'img' ||
        role === 'paragraph' ||
        role === 'code' ||
        role === 'separator' ||
        role === 'progressbar' ||
        role === 'meter' ||
        LANDMARKS.has(role) ||
        CONTAINER_ROLES.has(role);

      if (!keep) {
        return childrenOf(el);
      }
      if (!take()) return [];

      const node = { role, el };
      const name = accessibleName(el);
      if (name) node.name = clip(name, opts.maxText);
      if (interactive || role === 'heading') {
        node.ref = assignRef(el);
        refCount++;
      }
      const states = statesOf(el, role);
      if (Object.keys(states).length) node.states = states;
      if (role === 'heading') node.level = headingLevel(el);
      if (role === 'link') {
        const href = absoluteUrl(el, 'href');
        if (href) node.url = href;
      }
      if (role === 'img' && tag === 'img') {
        const src = absoluteUrl(el, 'src');
        if (src && !src.startsWith('data:')) node.url = src;
      }
      const value = currentValue(el);
      if (value !== null && value !== undefined && value !== '') {
        node.value = clip(value, opts.maxText);
      }
      if (interactive || role === 'heading') {
        const rect = topRect(el);
        if (!inViewport(rect)) node.offscreen = true;
      }
      if (tag === 'iframe' || tag === 'frame') return [frameNode(el, node)];

      // Name-from-content nodes already carry their text.
      const leafByName =
        NAME_FROM_CONTENT.has(role) && role !== 'listitem' && role !== 'cell' &&
        role !== 'columnheader' && role !== 'rowheader';
      if (role === 'paragraph' || role === 'code') {
        const text = textFromContent(el, 0);
        if (text && !node.name) node.name = clip(text, opts.maxText);
        const kids = Array.from(el.querySelectorAll('a[href],button,input,select,textarea,[role],[tabindex]'));
        if (kids.length) {
          const children = childrenOf(el).filter((c) => c.role !== 'text');
          if (children.length) node.children = children;
        }
        return [node];
      }
      if (!leafByName || isTextField(el)) {
        const children = childrenOf(el);
        if (children.length) node.children = children;
      } else {
        // Keep interactive descendants (e.g. a link inside a button-like row).
        const children = childrenOf(el).filter((c) => c.ref || (c.children && c.children.length));
        if (children.length) node.children = children;
      }
      // Drop empty unnamed containers.
      if (!interactive && CONTAINER_ROLES.has(role) && !node.name && !node.children) return [];
      return [node];
    }

    const whole = rootEl === document.body || rootEl === document.documentElement;
    const tree = whole ? childrenOf(rootEl) : visitElement(rootEl);
    return { tree, budget, refCount };
  }

  function quote(text) {
    return JSON.stringify(text);
  }

  function nodeLine(node) {
    let line = node.role;
    if (node.name) line += ' ' + quote(node.name);
    if (node.ref) line += ` [ref=${node.ref}]`;
    if (node.level) line += ` [level=${node.level}]`;
    const s = node.states || {};
    if (s.checked === true) line += ' [checked]';
    else if (s.checked === 'mixed') line += ' [checked=mixed]';
    else if (s.checked === false) line += ' [unchecked]';
    if (s.selected) line += ' [selected]';
    if (s.expanded === true) line += ' [expanded]';
    else if (s.expanded === false) line += ' [collapsed]';
    if (s.pressed) line += ' [pressed]';
    if (s.disabled) line += ' [disabled]';
    if (s.required) line += ' [required]';
    if (s.readonly) line += ' [readonly]';
    if (s.invalid) line += ' [invalid]';
    if (s.focused) line += ' [focused]';
    if (node.offscreen) line += ' [offscreen]';
    if (node.frame === 'cross-origin') line += ' [cross-origin: contents unavailable]';
    if (node.url) line += ` [url=${node.url}]`;
    if (node.value !== undefined) line += ': ' + quote(node.value);
    return line;
  }

  function renderText(tree, budget, maxChars) {
    const lines = [];
    let chars = 0;
    let cut = false;
    const walk = (nodes, depth) => {
      for (const node of nodes) {
        if (cut) return;
        const pad = '  '.repeat(depth);
        const line =
          node.role === 'text' ? `${pad}- text: ${quote(node.name)}` : `${pad}- ${nodeLine(node)}`;
        if (chars + line.length + 1 > maxChars) {
          cut = true;
          return;
        }
        lines.push(line);
        chars += line.length + 1;
        if (node.options) {
          for (const opt of node.options) {
            const o = `${pad}  - option ${quote(opt.label)}${opt.selected ? ' [selected]' : ''}${opt.disabled ? ' [disabled]' : ''}`;
            if (chars + o.length + 1 > maxChars) {
              cut = true;
              return;
            }
            lines.push(o);
            chars += o.length + 1;
          }
        }
        if (node.children) walk(node.children, depth + 1);
      }
    };
    walk(tree, 0);
    const truncated = cut || budget.truncated;
    if (truncated) {
      lines.push(
        `- … [truncated: snapshot exceeded ${cut ? 'max_chars' : 'max_nodes'}; pass root to snapshot a subtree]`,
      );
    }
    return { lines, truncated };
  }

  function jsonTree(nodes) {
    return nodes.map((node) => {
      const out = { role: node.role };
      for (const key of ['name', 'ref', 'level', 'value', 'url', 'offscreen', 'frame', 'states', 'options', 'optionsTruncated']) {
        if (node[key] !== undefined) out[key] = node[key];
      }
      if (node.children) out.children = jsonTree(node.children);
      return out;
    });
  }

  function lineDiff(before, after) {
    const n = before.length;
    const m = after.length;
    if (n * m > 4000000) {
      const a = new Set(before);
      const b = new Set(after);
      return {
        removed: before.filter((l) => !b.has(l)),
        added: after.filter((l) => !a.has(l)),
      };
    }
    const lcs = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
    for (let i = n - 1; i >= 0; i--) {
      for (let j = m - 1; j >= 0; j--) {
        lcs[i][j] = before[i] === after[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
      }
    }
    const added = [];
    const removed = [];
    let i = 0;
    let j = 0;
    while (i < n && j < m) {
      if (before[i] === after[j]) {
        i++;
        j++;
      } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
        removed.push(before[i++]);
      } else {
        added.push(after[j++]);
      }
    }
    while (i < n) removed.push(before[i++]);
    while (j < m) added.push(after[j++]);
    return { added, removed };
  }

  function snapshot(args) {
    args = args || {};
    let rootEl = document.body || document.documentElement;
    if (args.root) {
      const resolved = resolveTarget(args.root);
      if (resolved.error) return resolved.error;
      rootEl = resolved.el;
    }
    const opts = {
      maxNodes: args.max_nodes > 0 ? args.max_nodes : 2000,
      maxChars: args.max_chars > 0 ? args.max_chars : 40000,
      maxText: args.max_text > 0 ? args.max_text : 160,
    };
    const built = buildTree(rootEl, opts);
    const rendered = renderText(built.tree, built.budget, opts.maxChars);
    const scope = args.root ? 'root:' + JSON.stringify(args.root) : 'page';
    const previous = lastSnapshot && lastSnapshot.scope === scope ? lastSnapshot : null;
    lastSnapshot = { scope, lines: rendered.lines };
    const result = {
      url: location.href,
      title: document.title,
      truncated: rendered.truncated,
      node_count: built.budget.nodes,
      ref_count: built.refCount,
      viewport: {
        width: window.innerWidth,
        height: window.innerHeight,
        scroll_x: window.scrollX,
        scroll_y: window.scrollY,
        device_pixel_ratio: window.devicePixelRatio || 1,
      },
    };
    if (args.since === 'last') {
      if (!previous) {
        result.diff = null;
        result.diff_reason = 'no_previous_snapshot';
      } else {
        const diff = lineDiff(previous.lines, rendered.lines);
        result.diff = {
          unchanged: diff.added.length === 0 && diff.removed.length === 0,
          added: diff.added,
          removed: diff.removed,
          text: diff.removed.map((l) => '- ' + l.trimStart()).concat(diff.added.map((l) => '+ ' + l.trimStart())).join('\n'),
        };
        if (!args.full) return ok(result);
      }
    }
    const format = args.format || 'text';
    if (format === 'text' || format === 'both') result.text = rendered.lines.join('\n');
    if (format === 'json' || format === 'both') result.nodes = jsonTree(built.tree);
    return ok(result);
  }

  // ---------------------------------------------------------------------
  // Actionability

  const POINTER_ACTIONS = new Set(['click', 'hover', 'check', 'drag']);

  function probe(args) {
    const action = args.action || 'click';
    const resolved = resolveTarget(args.target);
    if (resolved.error) return resolved.error;
    const el = resolved.el;
    const ref = assignRef(el);
    const subject = describe(el);

    if (!isRendered(el) || !isVisible(el)) {
      // An <option> is hidden inside its <select>: select through the parent.
      return fail('not_visible', 'element is not visible', { target: subject });
    }
    if (action !== 'scroll_to' && action !== 'hover' && isDisabled(el)) {
      return fail('disabled', 'element is disabled', { target: subject });
    }
    if (action === 'fill' || action === 'type') {
      if (!isEditable(el)) {
        const why = tagOf(el) === 'select' ? 'use select_option for a <select>' : 'element is not editable';
        return fail('not_editable', why, { target: subject });
      }
    }
    if (args.scroll !== false) scrollIntoViewIfNeeded(el);
    const rect = topRect(el);
    if (rect.width <= 0 || rect.height <= 0) {
      return fail('not_visible', 'element has no size', { target: subject });
    }
    const point = {
      x: Math.min(Math.max(rect.x + rect.width / 2, 0), window.innerWidth - 1),
      y: Math.min(Math.max(rect.y + rect.height / 2, 0), window.innerHeight - 1),
    };
    if (!inViewport(rect)) {
      return fail('not_visible', 'element is outside the viewport after scrolling', {
        target: subject,
        rect,
      });
    }
    if (POINTER_ACTIONS.has(action)) {
      const cover = coveringElement(el, point);
      if (cover) {
        return fail('covered', 'another element would receive the click', {
          target: subject,
          covered_by: describe(cover),
          point,
        });
      }
    }
    let draggable = false;
    for (let n = el; n && n.nodeType === 1; n = n.parentElement) {
      if (n.draggable) { draggable = true; break; }
    }
    return ok({ ref, rect, point, target: subject, draggable });
  }

  // ---------------------------------------------------------------------
  // DOM-synthesized input (platforms without trusted pointer input)

  function pointerInit(el, point, extra) {
    return Object.assign(
      {
        bubbles: true,
        cancelable: true,
        composed: true,
        view: el.ownerDocument.defaultView,
        clientX: point ? point.x : 0,
        clientY: point ? point.y : 0,
        button: 0,
        buttons: 1,
      },
      extra || {},
    );
  }

  function localPoint(el, point) {
    const chain = frameChain(el) || [];
    let x = point.x;
    let y = point.y;
    for (const fe of chain) {
      const o = frameContentOffset(fe);
      x -= o.x;
      y -= o.y;
    }
    return { x, y };
  }

  function dispatchPointer(el, type, init) {
    const win = el.ownerDocument.defaultView;
    try {
      if (type.startsWith('pointer') && win.PointerEvent) {
        el.dispatchEvent(new win.PointerEvent(type, Object.assign({ pointerId: 1, isPrimary: true, pointerType: 'mouse' }, init)));
      } else {
        el.dispatchEvent(new win.MouseEvent(type, init));
      }
    } catch (_e) {}
  }

  function focusElement(el) {
    if (typeof el.focus === 'function') {
      try {
        el.focus({ preventScroll: true });
      } catch (_e) {
        try { el.focus(); } catch (__e) {}
      }
    }
  }

  function synthClick(args) {
    const resolved = resolveTarget(args.target);
    if (resolved.error) return resolved.error;
    const el = resolved.el;
    const point = localPoint(el, args.point || topCenter(el));
    const button = args.button === 'right' ? 2 : args.button === 'middle' ? 1 : 0;
    const count = args.click_count > 0 ? args.click_count : 1;
    const hit = deepElementFromPoint(el.ownerDocument, point.x, point.y);
    const dest = hit && composedContains(el, hit) ? hit : el;
    const base = pointerInit(dest, point, { button, buttons: button === 2 ? 2 : button === 1 ? 4 : 1 });
    dispatchPointer(dest, 'pointerover', base);
    dispatchPointer(dest, 'mouseover', base);
    dispatchPointer(dest, 'pointermove', base);
    dispatchPointer(dest, 'mousemove', base);
    for (let i = 1; i <= count; i++) {
      const init = Object.assign({}, base, { detail: i });
      dispatchPointer(dest, 'pointerdown', init);
      dispatchPointer(dest, 'mousedown', init);
      if (i === 1) focusElement(focusTarget(el));
      dispatchPointer(dest, 'pointerup', Object.assign({}, init, { buttons: 0 }));
      dispatchPointer(dest, 'mouseup', Object.assign({}, init, { buttons: 0 }));
      if (button === 2) {
        dispatchPointer(dest, 'contextmenu', init);
      } else if (button === 0) {
        // `click()` runs default actions (links, labels, checkboxes, submit).
        if (i === 1 && typeof dest.click === 'function' && dest === el) {
          el.click();
        } else {
          dispatchPointer(dest, 'click', Object.assign({}, init, { buttons: 0 }));
        }
        if (i === 2) dispatchPointer(dest, 'dblclick', Object.assign({}, init, { buttons: 0 }));
      } else {
        dispatchPointer(dest, 'auxclick', Object.assign({}, init, { buttons: 0 }));
      }
    }
    return ok({ trusted: false });
  }

  function focusTarget(el) {
    // Clicking a label focuses its control; otherwise the closest focusable.
    let n = el;
    while (n && n.nodeType === 1) {
      if (typeof n.tabIndex === 'number' && n.tabIndex >= 0) return n;
      n = n.parentElement;
    }
    return el;
  }

  function topCenter(el) {
    const r = topRect(el);
    return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
  }

  function synthHover(args) {
    const resolved = resolveTarget(args.target);
    if (resolved.error) return resolved.error;
    const el = resolved.el;
    const point = localPoint(el, args.point || topCenter(el));
    const init = pointerInit(el, point, { buttons: 0 });
    for (const type of ['pointerover', 'pointerenter', 'mouseover', 'mouseenter', 'pointermove', 'mousemove']) {
      const bubbles = !type.endsWith('enter');
      dispatchPointer(el, type, Object.assign({}, init, { bubbles }));
    }
    return ok({ trusted: false });
  }

  function synthDrag(args) {
    const src = resolveTarget(args.target);
    if (src.error) return src.error;
    const dst = resolveTarget(args.to);
    if (dst.error) return dst.error;
    const from = src.el;
    const to = dst.el;
    const a = localPoint(from, topCenter(from));
    const b = localPoint(to, topCenter(to));
    const win = from.ownerDocument.defaultView;
    dispatchPointer(from, 'pointerdown', pointerInit(from, a));
    dispatchPointer(from, 'mousedown', pointerInit(from, a));
    let dataTransfer = null;
    try {
      dataTransfer = new win.DataTransfer();
    } catch (_e) {}
    const dragInit = (pt) => Object.assign(pointerInit(from, pt), { dataTransfer });
    let draggable = false;
    for (let n = from; n && n.nodeType === 1; n = n.parentElement) {
      if (n.draggable) { draggable = true; break; }
    }
    if (draggable && win.DragEvent) {
      const fire = (node, type, pt) => {
        try {
          node.dispatchEvent(new win.DragEvent(type, dragInit(pt)));
        } catch (_e) {}
      };
      fire(from, 'dragstart', a);
      fire(to, 'dragenter', b);
      fire(to, 'dragover', b);
      fire(to, 'drop', b);
      fire(from, 'dragend', b);
    } else {
      const steps = 5;
      for (let i = 1; i <= steps; i++) {
        const pt = { x: a.x + ((b.x - a.x) * i) / steps, y: a.y + ((b.y - a.y) * i) / steps };
        const over = deepElementFromPoint(from.ownerDocument, pt.x, pt.y) || to;
        dispatchPointer(over, 'pointermove', pointerInit(over, pt));
        dispatchPointer(over, 'mousemove', pointerInit(over, pt));
      }
      dispatchPointer(to, 'pointerup', pointerInit(to, b, { buttons: 0 }));
      dispatchPointer(to, 'mouseup', pointerInit(to, b, { buttons: 0 }));
    }
    return ok({ trusted: false, html5: draggable });
  }

  // ---------------------------------------------------------------------
  // Form input

  function setNativeValue(el, value) {
    const win = el.ownerDocument.defaultView;
    const proto =
      tagOf(el) === 'textarea' ? win.HTMLTextAreaElement.prototype
        : tagOf(el) === 'select' ? win.HTMLSelectElement.prototype
          : win.HTMLInputElement.prototype;
    const desc = Object.getOwnPropertyDescriptor(proto, 'value');
    if (desc && desc.set) desc.set.call(el, value);
    else el.value = value;
  }

  function fireInputChange(el, data, inputType) {
    const win = el.ownerDocument.defaultView;
    try {
      el.dispatchEvent(new win.InputEvent('input', { bubbles: true, composed: true, data: data == null ? null : data, inputType: inputType || 'insertText' }));
    } catch (_e) {
      el.dispatchEvent(new win.Event('input', { bubbles: true, composed: true }));
    }
    el.dispatchEvent(new win.Event('change', { bubbles: true }));
  }

  function selectAllIn(el) {
    const doc = el.ownerDocument;
    const sel = doc.getSelection && doc.getSelection();
    if (!sel) return;
    const range = doc.createRange();
    range.selectNodeContents(el);
    sel.removeAllRanges();
    sel.addRange(range);
  }

  function fill(args) {
    const resolved = resolveTarget(args.target);
    if (resolved.error) return resolved.error;
    const el = resolved.el;
    const value = String(args.value == null ? '' : args.value);
    if (!isEditable(el)) {
      if (tagOf(el) === 'select') return fail('not_editable', 'use select_option for a <select>', { target: describe(el) });
      return fail('not_editable', 'element is not editable', { target: describe(el) });
    }
    focusElement(el);
    if (el.isContentEditable && !isTextField(el) && !isValueField(el)) {
      selectAllIn(el);
      const doc = el.ownerDocument;
      let inserted = false;
      try {
        inserted = value === '' ? doc.execCommand('delete', false) : doc.execCommand('insertText', false, value);
      } catch (_e) {
        inserted = false;
      }
      if (!inserted || collapse(el.innerText) !== collapse(value)) {
        el.textContent = value;
        fireInputChange(el, value, 'insertText');
      }
      return ok({ value: el.innerText });
    }
    if (isValueField(el)) {
      setNativeValue(el, value);
      if (el.value !== value && value !== '') {
        return fail('usage', `value ${JSON.stringify(value)} is not valid for <input type=${el.type}>`, {
          target: describe(el),
          hint: el.type === 'date' ? 'YYYY-MM-DD' : el.type === 'time' ? 'HH:MM' : el.type === 'datetime-local' ? 'YYYY-MM-DDTHH:MM' : el.type === 'month' ? 'YYYY-MM' : el.type === 'week' ? 'YYYY-Www' : el.type === 'color' ? '#rrggbb' : undefined,
        });
      }
      fireInputChange(el, value, 'insertReplacementText');
      return ok({ value: el.value });
    }
    if (typeof el.select === 'function') {
      try { el.select(); } catch (_e) {}
    }
    setNativeValue(el, value);
    fireInputChange(el, value, 'insertReplacementText');
    return ok({ value: el.type === 'password' ? undefined : el.value });
  }

  // Focus a text target with the caret at the end so native text input
  // lands there; `clear` empties it first.
  function focusForTyping(args) {
    const resolved = resolveTarget(args.target);
    if (resolved.error) return resolved.error;
    const el = resolved.el;
    if (!isEditable(el)) return fail('not_editable', 'element is not editable', { target: describe(el) });
    focusElement(el);
    const doc = el.ownerDocument;
    if (args.clear) {
      if (isTextField(el)) {
        setNativeValue(el, '');
        fireInputChange(el, null, 'deleteContentBackward');
      } else if (el.isContentEditable) {
        selectAllIn(el);
        try { doc.execCommand('delete', false); } catch (_e) { el.textContent = ''; }
      }
    }
    if (isTextField(el) && typeof el.setSelectionRange === 'function') {
      try {
        const end = el.value.length;
        el.setSelectionRange(end, end);
      } catch (_e) {}
    } else if (el.isContentEditable) {
      const sel = doc.getSelection();
      const range = doc.createRange();
      range.selectNodeContents(el);
      range.collapse(false);
      sel.removeAllRanges();
      sel.addRange(range);
    }
    return ok({ focused: doc.activeElement === el || el.contains(doc.activeElement) });
  }

  // Composition events around native text insertion, so frameworks that
  // buffer IME input (React onCompositionEnd, Vue v-model) see CJK text
  // arrive the way a real IME delivers it.
  function composition(args) {
    const doc = document;
    let el = doc.activeElement;
    while (el && el.shadowRoot && el.shadowRoot.activeElement) el = el.shadowRoot.activeElement;
    while (el && (tagOf(el) === 'iframe' || tagOf(el) === 'frame')) {
      try { el = el.contentDocument.activeElement; } catch (_e) { break; }
    }
    if (!el) return fail('not_found', 'no focused element');
    const win = el.ownerDocument.defaultView;
    const fire = (type, data) => {
      try {
        el.dispatchEvent(new win.CompositionEvent(type, { bubbles: true, cancelable: true, composed: true, data }));
      } catch (_e) {}
    };
    if (args.phase === 'start') {
      fire('compositionstart', '');
      fire('compositionupdate', args.text || '');
    } else {
      fire('compositionend', args.text || '');
    }
    return ok({});
  }

  // Typing without a browser-level keyboard: synthetic key events around
  // `execCommand('insertText')`, whose beforeinput/input events are trusted
  // and go through the editor (undo stack, maxlength, selection). Non-ASCII
  // text arrives inside a composition, as an IME commits it.
  function synthType(args) {
    const focus = focusForTyping({ target: args.target, clear: args.clear });
    if (!focus.ok) return focus;
    const el = resolveTarget(args.target).el;
    const win = el.ownerDocument.defaultView;
    const doc = el.ownerDocument;
    const text = String(args.text || '');
    const composed = /[^\x00-\x7f]/.test(text);
    if (composed) composition({ phase: 'start', text });
    let fallback = false;
    for (const ch of Array.from(text)) {
      const key = ch === '\n' ? 'Enter' : ch;
      const keyInit = { key, bubbles: true, cancelable: true, composed: true, isComposing: composed };
      const proceed = el.dispatchEvent(new win.KeyboardEvent('keydown', keyInit));
      let inserted = false;
      if (proceed) {
        try {
          inserted = ch === '\n' && tagOf(el) === 'input'
            ? true
            : doc.execCommand(ch === '\n' ? 'insertParagraph' : 'insertText', false, ch);
        } catch (_e) {
          inserted = false;
        }
      }
      if (proceed && !inserted) {
        fallback = true;
        if (isTextField(el)) {
          const start = el.selectionStart != null ? el.selectionStart : el.value.length;
          const end = el.selectionEnd != null ? el.selectionEnd : el.value.length;
          setNativeValue(el, el.value.slice(0, start) + ch + el.value.slice(end));
          try { el.setSelectionRange(start + ch.length, start + ch.length); } catch (_e) {}
          try {
            el.dispatchEvent(new win.InputEvent('input', { bubbles: true, composed: true, data: ch, inputType: composed ? 'insertCompositionText' : 'insertText', isComposing: composed }));
          } catch (_e) {}
        } else {
          try { el.ownerDocument.execCommand('insertText', false, ch); } catch (_e) {}
        }
      }
      el.dispatchEvent(new win.KeyboardEvent('keyup', keyInit));
    }
    if (composed) composition({ phase: 'end', text });
    if (fallback && isTextField(el)) el.dispatchEvent(new win.Event('change', { bubbles: true }));
    return ok({ trusted: false, value: isTextField(el) && el.type !== 'password' ? el.value : undefined });
  }

  function selectOption(args) {
    const resolved = resolveTarget(args.target);
    if (resolved.error) return resolved.error;
    const el = resolved.el;
    if (tagOf(el) !== 'select') {
      return fail('unsupported', 'select_option needs a <select>; for a custom listbox click the option ref', { target: describe(el) });
    }
    if (isDisabled(el)) return fail('disabled', 'element is disabled', { target: describe(el) });
    const wanted = Array.isArray(args.options) ? args.options : [];
    if (!wanted.length) return fail('usage', 'pass at least one option');
    if (wanted.length > 1 && !el.multiple) {
      return fail('usage', 'this <select> is not multiple; pass one option', { target: describe(el) });
    }
    const options = Array.from(el.options);
    const picked = [];
    for (const want of wanted) {
      let match = null;
      if (typeof want === 'string') {
        match = options.find((o) => o.value === want) ||
          options.find((o) => collapse(o.label || o.text) === collapse(want)) ||
          options.find((o) => collapse(o.label || o.text).toLowerCase() === collapse(want).toLowerCase());
      } else if (want.value !== undefined) match = options.find((o) => o.value === String(want.value));
      else if (want.label !== undefined) {
        const label = collapse(want.label);
        match = options.find((o) => collapse(o.label || o.text) === label) ||
          options.find((o) => collapse(o.label || o.text).toLowerCase() === label.toLowerCase());
      } else if (Number.isInteger(want.index)) match = options[want.index] || null;
      if (!match) {
        return fail('not_found', 'no option matches ' + JSON.stringify(want), {
          target: describe(el),
          available: options.slice(0, 50).map((o) => ({ label: collapse(o.label || o.text), value: o.value })),
        });
      }
      if (match.disabled) return fail('disabled', 'option is disabled: ' + collapse(match.label || match.text), { target: describe(el) });
      picked.push(match);
    }
    focusElement(el);
    for (const o of options) o.selected = picked.includes(o);
    const win = el.ownerDocument.defaultView;
    el.dispatchEvent(new win.Event('input', { bubbles: true, composed: true }));
    el.dispatchEvent(new win.Event('change', { bubbles: true }));
    return ok({
      selected: Array.from(el.selectedOptions).map((o) => ({ label: collapse(o.label || o.text), value: o.value, index: o.index })),
    });
  }

  function checkable(el) {
    const tag = tagOf(el);
    if (tag === 'input' && (el.type === 'checkbox' || el.type === 'radio')) return true;
    const role = roleOf(el);
    return role === 'checkbox' || role === 'radio' || role === 'switch' ||
      role === 'menuitemcheckbox' || role === 'menuitemradio';
  }

  // The checkbox a target stands for: itself, or the control of a label.
  function checkTarget(args) {
    const resolved = resolveTarget(args.target);
    if (resolved.error) return resolved.error;
    let el = resolved.el;
    if (!checkable(el) && tagOf(el) === 'label' && el.control) el = el.control;
    if (!checkable(el)) {
      return fail('unsupported', 'element is not a checkbox, radio, or switch', { target: describe(el) });
    }
    const state = checkedState(el, roleOf(el));
    return ok({ ref: assignRef(el), checked: state === true, mixed: state === 'mixed', radio: el.type === 'radio' || roleOf(el) === 'radio' || roleOf(el) === 'menuitemradio' });
  }

  // A visible label that toggles a (visually hidden) checkbox or radio.
  function labelFor(args) {
    const resolved = resolveTarget(args.target);
    if (resolved.error) return resolved.error;
    const el = resolved.el;
    const labels = Array.from(el.labels || []);
    const wrapping = el.closest && el.closest('label');
    if (wrapping && !labels.includes(wrapping)) labels.push(wrapping);
    const label = labels.find(isVisible);
    if (!label) return fail('not_visible', 'the control and its labels are hidden', { target: describe(el) });
    return ok({ label: assignRef(label) });
  }

  function setFiles(args) {
    const resolved = resolveTarget(args.target);
    if (resolved.error) return resolved.error;
    const el = resolved.el;
    if (tagOf(el) !== 'input' || (el.type || '').toLowerCase() !== 'file') {
      return fail('unsupported', 'set_input_files needs an <input type=file>', { target: describe(el) });
    }
    if (isDisabled(el)) return fail('disabled', 'element is disabled', { target: describe(el) });
    const files = Array.isArray(args.files) ? args.files : [];
    if (files.length > 1 && !el.multiple) {
      return fail('usage', 'this file input is not multiple; pass one file', { target: describe(el) });
    }
    const win = el.ownerDocument.defaultView;
    let dt;
    try {
      dt = new win.DataTransfer();
    } catch (_e) {
      return fail('unsupported', 'this WebView cannot construct a FileList');
    }
    for (const f of files) {
      const bin = atob(f.base64 || '');
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      dt.items.add(new win.File([bytes], f.name, { type: f.mime || '', lastModified: f.last_modified || Date.now() }));
    }
    el.files = dt.files;
    el.dispatchEvent(new win.Event('input', { bubbles: true, composed: true }));
    el.dispatchEvent(new win.Event('change', { bubbles: true }));
    return ok({ files: Array.from(el.files).map((f) => ({ name: f.name, size: f.size, type: f.type })) });
  }

  function focusOp(args) {
    const resolved = resolveTarget(args.target);
    if (resolved.error) return resolved.error;
    focusElement(resolved.el);
    return ok({});
  }

  function scrollTo(args) {
    const resolved = resolveTarget(args.target);
    if (resolved.error) return resolved.error;
    scrollIntoViewIfNeeded(resolved.el);
    return ok({ rect: topRect(resolved.el) });
  }

  function elementBox(args) {
    const resolved = resolveTarget(args.target);
    if (resolved.error) return resolved.error;
    const el = resolved.el;
    if (!isVisible(el)) return fail('not_visible', 'element is not visible', { target: describe(el) });
    const rect = topRect(el);
    return ok({
      rect,
      document: { x: rect.x + window.scrollX, y: rect.y + window.scrollY, width: rect.width, height: rect.height },
      ref: assignRef(el),
    });
  }

  function metrics() {
    const de = document.documentElement;
    const body = document.body;
    return ok({
      width: Math.max(de.scrollWidth, body ? body.scrollWidth : 0, de.clientWidth),
      height: Math.max(de.scrollHeight, body ? body.scrollHeight : 0, de.clientHeight),
      viewport_width: window.innerWidth,
      viewport_height: window.innerHeight,
      scroll_x: window.scrollX,
      scroll_y: window.scrollY,
      device_pixel_ratio: window.devicePixelRatio || 1,
    });
  }

  // Resolves when the DOM has been quiet for `quiet_ms`, or after `max_ms`.
  function settle(args) {
    const quiet = args.quiet_ms > 0 ? args.quiet_ms : 150;
    const max = args.max_ms > 0 ? args.max_ms : 2000;
    return new Promise((resolve) => {
      const started = Date.now();
      let timer = 0;
      const done = () => {
        observer.disconnect();
        clearTimeout(timer);
        clearTimeout(cap);
        resolve(ok({ elapsed_ms: Date.now() - started }));
      };
      const observer = new MutationObserver(() => {
        clearTimeout(timer);
        timer = setTimeout(done, quiet);
      });
      observer.observe(document, { subtree: true, childList: true, attributes: true, characterData: true });
      timer = setTimeout(done, quiet);
      const cap = setTimeout(done, max);
    });
  }

  // After the next paint; a timer stands in where frames are paused.
  function frame() {
    return new Promise((resolve) => {
      const done = () => resolve(ok({}));
      const timer = setTimeout(done, 100);
      requestAnimationFrame(() => requestAnimationFrame(() => { clearTimeout(timer); done(); }));
    });
  }

  // ---------------------------------------------------------------------
  // Readable text as Markdown

  const BLOCK_SKIP = new Set(['script', 'style', 'noscript', 'template', 'svg', 'canvas', 'iframe', 'head', 'select', 'button', 'input', 'textarea', 'form']);

  function pickMainRoot() {
    const main = document.querySelector('main, [role=main]');
    if (main && isRendered(main)) return main;
    const articles = Array.from(document.querySelectorAll('article')).filter(isRendered);
    if (articles.length === 1) return articles[0];
    return document.body || document.documentElement;
  }

  function textMarkdown(args) {
    let root;
    if (args.target) {
      const resolved = resolveTarget(args.target);
      if (resolved.error) return resolved.error;
      root = resolved.el;
    } else {
      root = args.all ? document.body || document.documentElement : pickMainRoot();
    }
    const maxChars = args.max_chars > 0 ? args.max_chars : 20000;
    const skipChrome = !args.target && !args.all && (root === document.body || root === document.documentElement);
    const out = [];
    let inline = '';
    const flush = () => {
      const t = inline.replace(/[ \t]+/g, ' ').replace(/ *\n */g, '\n').trim();
      if (t) out.push(t);
      inline = '';
    };
    const esc = (t) => t.replace(/([\\`*_\[\]])/g, '\\$1');

    function inlineText(node) {
      let s = '';
      const walk = (n) => {
        if (n.nodeType === 3) { s += esc(n.nodeValue.replace(/\s+/g, ' ')); return; }
        if (n.nodeType !== 1) return;
        const tag = tagOf(n);
        if (BLOCK_SKIP.has(tag) && tag !== 'button') return;
        if (!isRendered(n)) return;
        if (tag === 'br') { s += '\n'; return; }
        if (tag === 'img') { const alt = n.getAttribute('alt'); if (alt) s += `![${esc(alt)}]`; return; }
        if (tag === 'code') { s += '`' + n.textContent + '`'; return; }
        let inner = '';
        const saved = s; s = '';
        for (const c of (n.shadowRoot || n).childNodes) walk(c);
        inner = s; s = saved;
        if (tag === 'a' && n.getAttribute('href')) {
          const href = absoluteUrl(n, 'href');
          const label = inner.trim();
          s += label ? `[${label}](${href})` : '';
        } else if ((tag === 'strong' || tag === 'b') && inner.trim()) {
          s += `**${inner.trim()}**`;
        } else if ((tag === 'em' || tag === 'i') && inner.trim()) {
          s += `_${inner.trim()}_`;
        } else {
          s += inner;
        }
      };
      walk(node);
      return s;
    }

    function table(el) {
      const rows = Array.from(el.querySelectorAll('tr')).filter((tr) => tr.closest('table') === el);
      if (!rows.length) return;
      const cells = rows.map((tr) => Array.from(tr.children).filter((c) => /^t[hd]$/.test(tagOf(c))).map((c) => inlineText(c).replace(/\n/g, ' ').replace(/\|/g, '\\|').trim()));
      const width = Math.max(...cells.map((r) => r.length));
      if (!width) return;
      const pad = (r) => r.concat(Array(width - r.length).fill(''));
      const lines = [];
      lines.push('| ' + pad(cells[0]).join(' | ') + ' |');
      lines.push('|' + Array(width).fill(' --- ').join('|') + '|');
      for (const r of cells.slice(1)) lines.push('| ' + pad(r).join(' | ') + ' |');
      out.push(lines.join('\n'));
    }

    function list(el, depth) {
      const ordered = tagOf(el) === 'ol';
      let index = 1;
      const lines = [];
      for (const li of el.children) {
        if (tagOf(li) !== 'li' || !isRendered(li)) continue;
        let own = '';
        const nested = [];
        for (const c of li.childNodes) {
          if (c.nodeType === 1 && (tagOf(c) === 'ul' || tagOf(c) === 'ol')) nested.push(c);
          else own += c.nodeType === 1 ? inlineText(c) : c.nodeType === 3 ? esc(c.nodeValue.replace(/\s+/g, ' ')) : '';
        }
        lines.push('  '.repeat(depth) + (ordered ? `${index++}. ` : '- ') + own.replace(/\s+/g, ' ').trim());
        for (const n of nested) lines.push(list(n, depth + 1));
      }
      return lines.join('\n');
    }

    function block(el) {
      if (el.nodeType === 3) { inline += esc(el.nodeValue); return; }
      if (el.nodeType !== 1) return;
      const tag = tagOf(el);
      if (BLOCK_SKIP.has(tag)) return;
      if (!isRendered(el) || el.getAttribute('aria-hidden') === 'true') return;
      const role = roleOf(el);
      if (skipChrome && (role === 'navigation' || role === 'banner' || role === 'contentinfo' || role === 'complementary')) return;
      const m = /^h([1-6])$/.exec(tag);
      if (m || role === 'heading') {
        flush();
        const level = m ? Number(m[1]) : headingLevel(el);
        const t = inlineText(el).replace(/\s+/g, ' ').trim();
        if (t) out.push('#'.repeat(level) + ' ' + t);
        return;
      }
      if (tag === 'ul' || tag === 'ol') { flush(); const l = list(el, 0); if (l.trim()) out.push(l); return; }
      if (tag === 'table') { flush(); table(el); return; }
      if (tag === 'pre') { flush(); out.push('```\n' + el.textContent.replace(/\n$/, '') + '\n```'); return; }
      if (tag === 'blockquote') { flush(); const t = inlineText(el).trim(); if (t) out.push(t.split('\n').map((l) => '> ' + l).join('\n')); return; }
      if (tag === 'hr') { flush(); out.push('---'); return; }
      const display = styleOf(el).display;
      const isBlock = display === 'block' || display === 'flex' || display === 'grid' || display === 'list-item' || display === 'table' || tag === 'p' || tag === 'div' || tag === 'section' || tag === 'article';
      if (!isBlock) { inline += inlineText(el); return; }
      flush();
      const kids = el.shadowRoot ? el.shadowRoot.childNodes : el.childNodes;
      for (const c of kids) block(c);
      flush();
    }

    block(root);
    flush();
    let text = out.join('\n\n').replace(/\n{3,}/g, '\n\n');
    const total = text.length;
    let truncated = false;
    if (text.length > maxChars) {
      text = text.slice(0, maxChars) + `\n\n… [truncated: ${total - maxChars} more characters]`;
      truncated = true;
    }
    return ok({ url: location.href, title: document.title, markdown: text, truncated, total_chars: total });
  }

  // ---------------------------------------------------------------------
  // Set-of-marks overlay

  function marks(args) {
    clearMarks();
    const max = args.max_marks > 0 ? args.max_marks : 300;
    // Marks are not a snapshot the caller saw: keep the diff baseline.
    const baseline = lastSnapshot;
    const result = snapshot({ format: 'json', max_nodes: args.max_nodes || 5000, max_chars: 1e9 });
    lastSnapshot = baseline;
    if (!result.ok) return result;
    const items = [];
    const walk = (nodes) => {
      for (const n of nodes) {
        if (n.ref && !n.offscreen) items.push(n);
        if (n.children) walk(n.children);
      }
    };
    walk(result.nodes || []);
    const host = document.createElement('lingxia-agent-marks');
    host.setAttribute('aria-hidden', 'true');
    host.style.cssText = 'position:fixed;inset:0;z-index:2147483647;pointer-events:none;display:block;';
    const shadow = host.attachShadow({ mode: 'closed' });
    const palette = ['#e6194b', '#3cb44b', '#4363d8', '#f58231', '#911eb4', '#008080', '#9a6324', '#800000', '#000075', '#f032e6'];
    const out = [];
    for (const n of items.slice(0, max)) {
      const weak = byRef.get(n.ref);
      const el = weak && weak.deref();
      if (!el) continue;
      const r = topRect(el);
      // A visually hidden control (styled checkbox) would mark a dot.
      if (r.width < 4 || r.height < 4 || !inViewport(r)) continue;
      const color = palette[out.length % palette.length];
      const box = document.createElement('div');
      box.style.cssText = `position:fixed;left:${r.x}px;top:${r.y}px;width:${r.width}px;height:${r.height}px;border:2px solid ${color};box-sizing:border-box;border-radius:2px;`;
      const label = document.createElement('div');
      label.textContent = n.ref;
      const above = r.y >= 14;
      label.style.cssText = `position:fixed;left:${Math.max(0, r.x)}px;top:${above ? r.y - 14 : r.y}px;background:${color};color:#fff;font:bold 11px/14px -apple-system,Segoe UI,sans-serif;padding:0 3px;border-radius:2px;white-space:nowrap;`;
      shadow.appendChild(box);
      shadow.appendChild(label);
      const entry = { ref: n.ref, role: n.role, box: { x: r.x, y: r.y, width: r.width, height: r.height } };
      if (n.name) entry.name = n.name;
      out.push(entry);
    }
    (document.body || document.documentElement).appendChild(host);
    marksHost = host;
    return ok({
      marks: out,
      truncated: items.length > max,
      viewport: { width: window.innerWidth, height: window.innerHeight, device_pixel_ratio: window.devicePixelRatio || 1 },
    });
  }

  function clearMarks() {
    if (marksHost) {
      marksHost.remove();
      marksHost = null;
    }
    return ok({});
  }

  // ---------------------------------------------------------------------

  const OPS = {
    snapshot,
    probe,
    click: synthClick,
    hover: synthHover,
    drag: synthDrag,
    fill,
    focus: focusOp,
    focus_for_typing: focusForTyping,
    composition,
    type: synthType,
    select_option: selectOption,
    check_target: checkTarget,
    label_for: labelFor,
    set_files: setFiles,
    scroll_to: scrollTo,
    element_box: elementBox,
    metrics,
    settle,
    text: textMarkdown,
    marks,
    clear_marks: clearMarks,
    console: () => ok({}),
    frame,
  };

  // New page-console entries ride along with every result, so the host
  // keeps them after this document is gone.
  function harvest(result) {
    const recorder = window.__LingXiaPageConsole;
    if (!recorder || !result || typeof result !== 'object') return result;
    const taken = recorder.take(consoleCursor);
    if (taken.entries.length) {
      consoleCursor = taken.entries[taken.entries.length - 1].seq;
      result.console = taken.entries;
      result.console_url = location.href;
    }
    result.console_dropped = taken.dropped;
    return result;
  }

  Object.defineProperty(window, '__LingXiaAgent', {
    configurable: true,
    enumerable: false,
    value: Object.freeze({
      version: VERSION,
      generation,
      call(op, args) {
        const fn = OPS[op];
        if (!fn) return fail('usage', 'unknown agent op: ' + op);
        let result;
        try {
          result = fn(args || {});
        } catch (err) {
          result = fail('script', String(err && err.message ? err.message : err));
        }
        return result && typeof result.then === 'function' ? result.then(harvest) : harvest(result);
      },
    }),
  });
})();
