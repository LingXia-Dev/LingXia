import React, { forwardRef, useCallback, useId, useRef, useState } from 'react';
import { registerPickerComponent } from '../picker.js';

export interface LxPickerProps {
  columns: string[][] | [string[], Record<string, string[]>];
  value?: string | string[];
  onConfirm?: (value: string | string[]) => void;
  onCancel?: () => void;
  onScroll?: (value: string | string[]) => void;
  placeholder?: string;
  className?: string;
  style?: React.CSSProperties;
  disabled?: boolean;
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

export const LxPicker = forwardRef<HTMLElement, LxPickerProps>(({
  columns, value, onConfirm, onCancel, onScroll, placeholder = 'Please select',
  className, style, disabled, cancelText, cancelTextColor, cancelButtonColor,
  confirmText, confirmTextColor, confirmButtonColor, children
}, ref) => {
  const [visible, setVisible] = useState(false);
  const reactId = useId();
  const pickerId = `lx-picker-${reactId.replace(/[:]/g, "")}`;

  // Store latest props in refs to avoid stale closures
  const propsRef = useRef({ onConfirm, onCancel, onScroll, columns });
  propsRef.current = { onConfirm, onCancel, onScroll, columns };

  // Track if we've bound listeners to avoid duplicates
  const boundRef = useRef<HTMLElement | null>(null);

  const isCascading = columns.length === 2 && typeof columns[1] === 'object' && !Array.isArray(columns[1]);
  const isSingle = columns.length === 1;

  const getIndexFromValue = (): number | number[] => {
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
    return typeof value === 'string' ? value : value.join(' - ');
  };

  // Event handler - dispatch to appropriate callback based on flags
  const handleChange = useCallback((e: Event) => {
    const detail = (e as CustomEvent).detail;
    if (!detail) return;

    if (detail.confirmed) {
      setVisible(false);
      if (detail.index !== undefined) {
        propsRef.current.onConfirm?.(getValueFromIndex(propsRef.current.columns, detail.index));
      }
    } else if (detail.cancelled) {
      setVisible(false);
      propsRef.current.onCancel?.();
    } else if (detail.index !== undefined) {
      propsRef.current.onScroll?.(getValueFromIndex(propsRef.current.columns, detail.index));
    }
  }, []);

  // Stable ref callback - only binds/unbinds when element changes
  const pickerRefCallback = useCallback((el: HTMLElement | null) => {
    // Forward ref
    if (typeof ref === 'function') ref(el);
    else if (ref) (ref as React.MutableRefObject<HTMLElement | null>).current = el;

    // Unbind from old element
    if (boundRef.current && boundRef.current !== el) {
      boundRef.current.removeEventListener('change', handleChange);
      boundRef.current = null;
    }

    // Bind to new element
    if (el && boundRef.current !== el) {
      el.addEventListener('change', handleChange);
      boundRef.current = el;
    }
  }, [ref, handleChange]);

  const handleClick = () => !disabled && setVisible(true);

  const pickerProps: Record<string, string> = {
    id: pickerId,
    mode: isCascading ? 'cascading' : (isSingle ? 'selector' : 'multiSelector'),
    columns: JSON.stringify(columns),
    'default-index': JSON.stringify(getIndexFromValue()),
  };
  if (cancelText) pickerProps['cancel-text'] = cancelText;
  if (cancelTextColor) pickerProps['cancel-text-color'] = cancelTextColor;
  if (cancelButtonColor) pickerProps['cancel-button-color'] = cancelButtonColor;
  if (confirmText) pickerProps['confirm-text'] = confirmText;
  if (confirmTextColor) pickerProps['confirm-text-color'] = confirmTextColor;
  if (confirmButtonColor) pickerProps['confirm-button-color'] = confirmButtonColor;

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
