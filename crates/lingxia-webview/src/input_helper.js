(function() {
    if (window.__LingXiaInput) return;

    function findElement(selector, index) {
        if (typeof selector !== 'string' || selector.trim() === '') {
            return { el: null, count: 0 };
        }
        try {
            const nodes = Array.from(document.querySelectorAll(selector));
            const resolvedIndex = Number.isInteger(index) && index >= 0 ? index : 0;
            return { el: nodes[resolvedIndex] || null, count: nodes.length, index: resolvedIndex };
        } catch (_err) {
            return { el: null, count: 0 };
        }
    }

    function isEditable(el) {
        if (!el) return false;
        if (el.isContentEditable) return true;
        const tag = (el.tagName || '').toLowerCase();
        if (tag === 'textarea') {
            return !el.disabled && !el.readOnly;
        }
        if (tag === 'input') {
            const type = (el.type || 'text').toLowerCase();
            const blocked = new Set(['button', 'checkbox', 'color', 'file', 'hidden', 'image', 'radio', 'range', 'reset', 'submit']);
            return !el.disabled && !el.readOnly && !blocked.has(type);
        }
        return false;
    }

    function rectPayload(el) {
        const rect = el.getBoundingClientRect();
        const visible = rect.width > 0 &&
            rect.height > 0 &&
            rect.bottom > 0 &&
            rect.right > 0 &&
            rect.top < window.innerHeight &&
            rect.left < window.innerWidth;
        return {
            left: rect.left,
            top: rect.top,
            width: rect.width,
            height: rect.height,
            centerX: rect.left + (rect.width / 2),
            centerY: rect.top + (rect.height / 2),
            viewportWidth: window.innerWidth,
            viewportHeight: window.innerHeight,
            visible,
            editable: isEditable(el)
        };
    }

    function elementResult(selector, index) {
        const found = findElement(selector, index);
        const el = found.el;
        if (!el) {
            return { ok: false, error: `Element not found: ${selector}`, count: found.count, index: found.index || 0 };
        }
        return { ok: true, count: found.count, index: found.index || 0, ...rectPayload(el) };
    }

    window.__LingXiaInput = {
        query_box(selector, index) {
            return elementResult(selector, index);
        },
        is_visible(selector, index) {
            const result = elementResult(selector, index);
            return result.ok ? { ok: true, visible: result.visible } : result;
        },
        is_editable(selector, index) {
            const result = elementResult(selector, index);
            return result.ok ? { ok: true, editable: result.editable } : result;
        },
        focus(selector, index) {
            const found = findElement(selector, index);
            if (!found.el) {
                return { ok: false, error: `Element not found: ${selector}`, count: found.count, index: found.index || 0 };
            }
            if (typeof found.el.focus !== 'function') {
                return { ok: false, error: `Element cannot be focused: ${selector}`, count: found.count, index: found.index || 0 };
            }
            try {
                found.el.focus({ preventScroll: true });
            } catch (_err) {
                found.el.focus();
            }
            return document.activeElement === found.el
                ? { ok: true, count: found.count, index: found.index || 0 }
                : { ok: false, error: `Element did not accept focus: ${selector}`, count: found.count, index: found.index || 0 };
        }
    };
})();
