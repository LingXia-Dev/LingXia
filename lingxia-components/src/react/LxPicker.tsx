import React, { forwardRef, useCallback, useId, useRef, useState } from 'react';
import { registerPickerComponent } from '../picker.js';

export interface LxPickerProps extends Omit<React.HTMLAttributes<HTMLElement>, 'onChange' | 'onScroll'> {
  // For selector/multiSelector/cascading mode
  columns?: string[][] | [string[], Record<string, string[]>];

  // For date/time mode
  mode?: 'date' | 'time';
  start?: string;   // Valid date range start: 'YYYY-MM-DD'
  end?: string;     // Valid date range end: 'YYYY-MM-DD'
  fields?: 'year' | 'month' | 'day' | 'range';

  // Value (type depends on mode)
  value?: string | string[];

  // Callbacks
  onConfirm?: (value: string | string[]) => void;
  onCancel?: () => void;
  onScroll?: (value: string | string[]) => void;
  onChange?: (event: CustomEvent) => void;
  onNativeScroll?: (event: CustomEvent) => void;
  bindChange?: string;
  bindScroll?: string;
  catchChange?: string;
  catchScroll?: string;

  // UI
  placeholder?: string;
  className?: string;
  style?: React.CSSProperties;
  disabled?: boolean;

  // Button customization
  cancelText?: string;
  cancelTextColor?: string;
  cancelButtonColor?: string;
  confirmText?: string;
  confirmTextColor?: string;
  confirmButtonColor?: string;

  children?: React.ReactNode;
}

if (typeof window !== "undefined") {
  registerPickerComponent();
}

function normalizeBindingAttrName(key: string): string {
  return key.replace(/[^a-zA-Z0-9]/g, "").toLowerCase();
}

export const LxPicker = forwardRef<HTMLElement, LxPickerProps>(({
  columns, mode, start, end, fields, value, onConfirm, onCancel, onScroll, placeholder = 'Please select',
  onChange, onNativeScroll, bindChange, bindScroll, catchChange, catchScroll,
  className, style, disabled, cancelText, cancelTextColor, cancelButtonColor,
  confirmText, confirmTextColor, confirmButtonColor, children,
  ...rest
}, ref) => {
  const [visible, setVisible] = useState(false);
  const reactId = useId();
  const pickerId = `lx-picker-${reactId.replace(/[:]/g, "")}`;

  // Store latest props in refs to avoid stale closures
  const propsRef = useRef({ onConfirm, onCancel, onScroll, columns });
  propsRef.current = { onConfirm, onCancel, onScroll, columns };

  // Track if we've bound listeners to avoid duplicates
  const boundRef = useRef<HTMLElement | null>(null);
  const boundListenersRef = useRef<{
    change: EventListenerObject;
    scroll: EventListenerObject;
  } | null>(null);

  const isDateMode = mode === 'date' || mode === 'time';
  const isCascading = columns && columns.length === 2 && typeof columns[1] === 'object' && !Array.isArray(columns[1]);
  const isSingle = columns && columns.length === 1;

  const getIndexFromValue = (): number | number[] => {
    if (!columns) return 0;
    if (isSingle) {
      if (!value || typeof value !== 'string') return 0;
      const idx = (columns[0] as string[]).indexOf(value);
      return idx >= 0 ? idx : 0;
    }
    if (!value || !Array.isArray(value)) {
      return Array.from({ length: columns.length }, () => 0);
    }
    if (isCascading) {
      const [keys, map] = columns as [string[], Record<string, string[]>];
      const keyIdx = Math.max(0, keys.indexOf(value[0]));
      const valIdx = Math.max(0, map[keys[keyIdx]]?.indexOf(value[1]) ?? 0);
      return [keyIdx, valIdx];
    }
    const idxs = value.map((v, i) => Math.max(0, (columns[i] as string[])?.indexOf(v) ?? 0));
    while (idxs.length < columns.length) idxs.push(0);
    return idxs;
  };

  const getValueFromIndex = (cols: typeof columns, index: number | number[]): string | string[] => {
    if (!cols) return '';
    const cascading = cols.length === 2 && typeof cols[1] === 'object' && !Array.isArray(cols[1]);
    if (typeof index === 'number') return (cols[0] as string[])[index] ?? '';
    if (cascading) {
      const [keys, map] = cols as [string[], Record<string, string[]>];
      const key = keys[index[0]] ?? '';
      return [key, map[key]?.[index[1]] ?? ''];
    }
    return index.map((idx, col) => (cols[col] as string[])?.[idx] ?? '');
  };

  const displayText = (): string => {
    if (!value) return '';
    if (fields === 'range' && Array.isArray(value)) {
      return `${value[0]} ~ ${value[1]}`;
    }
    return typeof value === 'string' ? value : value.join(' - ');
  };

  // Event handler - dispatch to appropriate callback based on flags
  const handleChange = useCallback((e: Event) => {
    const detail = (e as CustomEvent).detail;
    if (typeof onChange === 'function') {
      onChange(e as CustomEvent);
    }
    if (!detail) return;

    if (detail.confirmed) {
      if (mode === 'date' || mode === 'time') {
        propsRef.current.onConfirm?.(detail.value);
      } else if (detail.index !== undefined) {
        propsRef.current.onConfirm?.(getValueFromIndex(propsRef.current.columns, detail.index));
      }
      setVisible(false);
    } else if (detail.cancelled) {
      propsRef.current.onCancel?.();
      setVisible(false);
    }
  }, [mode, onChange]);

  const handleScroll = useCallback((e: Event) => {
    const detail = (e as CustomEvent).detail;
    if (typeof onNativeScroll === 'function') {
      onNativeScroll(e as CustomEvent);
    }
    if (!detail) return;

    if (detail.value !== undefined) {
      propsRef.current.onScroll?.(detail.value);
    } else if (detail.index !== undefined) {
      propsRef.current.onScroll?.(getValueFromIndex(propsRef.current.columns, detail.index));
    }
  }, [mode, onNativeScroll]);

  // Stable ref callback - only binds/unbinds when element changes
  const pickerRefCallback = useCallback((el: HTMLElement | null) => {
    if (typeof ref === 'function') ref(el);
    else if (ref) (ref as React.MutableRefObject<HTMLElement | null>).current = el;

    if (boundRef.current && boundRef.current !== el && boundListenersRef.current) {
      boundRef.current.removeEventListener('change', boundListenersRef.current.change);
      boundRef.current.removeEventListener('scroll', boundListenersRef.current.scroll);
      boundRef.current = null;
      boundListenersRef.current = null;
    }

    if (el && boundRef.current !== el) {
      const listeners = {
        change: { handleEvent: (event: Event) => handleChange(event) } as EventListenerObject,
        scroll: { handleEvent: (event: Event) => handleScroll(event) } as EventListenerObject,
      };
      el.addEventListener('change', listeners.change);
      el.addEventListener('scroll', listeners.scroll);
      boundRef.current = el;
      boundListenersRef.current = listeners;
    }
  }, [ref, handleChange, handleScroll]);

  const handleClick = () => !disabled && setVisible(true);

  const pickerProps: Record<string, string> = {
    id: pickerId,
  };

  // Date mode
  if (isDateMode) {
    pickerProps.mode = mode!;
    if (fields) pickerProps.fields = fields;
    if (value) pickerProps.value = typeof value === 'string' ? value : JSON.stringify(value);
    if (start) pickerProps.start = start;
    if (end) pickerProps.end = end;
  } else {
    pickerProps.mode = isCascading ? 'cascading' : (isSingle ? 'selector' : 'multiSelector');
    pickerProps.columns = JSON.stringify(columns ?? []);
    pickerProps['default-index'] = JSON.stringify(getIndexFromValue());
  }

  // Common button props
  if (cancelText) pickerProps['cancel-text'] = cancelText;
  if (cancelTextColor) pickerProps['cancel-text-color'] = cancelTextColor;
  if (cancelButtonColor) pickerProps['cancel-button-color'] = cancelButtonColor;
  if (confirmText) pickerProps['confirm-text'] = confirmText;
  if (confirmTextColor) pickerProps['confirm-text-color'] = confirmTextColor;
  if (confirmButtonColor) pickerProps['confirm-button-color'] = confirmButtonColor;
  if (bindChange) pickerProps.bindchange = bindChange;
  if (bindScroll) pickerProps.bindscroll = bindScroll;
  if (catchChange) pickerProps.catchchange = catchChange;
  if (catchScroll) pickerProps.catchscroll = catchScroll;
  for (const [key, raw] of Object.entries(rest as Record<string, unknown>)) {
    if (raw === undefined || raw === null) continue;
    if (key.startsWith('data-')) {
      pickerProps[key] = String(raw);
      continue;
    }
    if ((key.startsWith('bind') || key.startsWith('catch')) && typeof raw === 'string') {
      pickerProps[normalizeBindingAttrName(key)] = raw;
    }
  }

  return (
    <>
      {children ? (
        <div onClick={handleClick} style={{ cursor: disabled ? 'not-allowed' : 'pointer' }}>{children}</div>
      ) : (
        <div
          onClick={handleClick}
          className={className}
          style={{
            display: 'flex', alignItems: 'center', justifyContent: 'space-between',
            padding: '12px 14px', backgroundColor: '#fff', border: '1px solid #e5e7eb',
            borderRadius: '8px', cursor: disabled ? 'not-allowed' : 'pointer',
            opacity: disabled ? 0.5 : 1, width: '100%', boxSizing: 'border-box',
            ...style,
          }}
        >
          <span style={{ color: value ? '#111' : '#9ca3af' }}>{displayText() || placeholder}</span>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#9ca3af" strokeWidth="2">
            <path d="M6 9l6 6 6-6" />
          </svg>
        </div>
      )}
      {visible && React.createElement('lx-picker', { ref: pickerRefCallback, ...pickerProps })}
    </>
  );
});

LxPicker.displayName = 'LxPicker';
