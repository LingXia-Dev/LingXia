import React, { forwardRef, useCallback, useId, useRef, useState } from 'react';
import { registerPickerComponent } from '../picker.js';
import {
  buildPickerNativeAttrs,
  getPickerDisplayText,
  getPickerValueFromIndex,
} from '../native_component_wrapper_shared.js';
import {
  assignForwardedRef,
  bindElementEvents,
  getCustomEventDetail,
  unbindElementEvents,
} from './text_component_shared.js';

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

export const LxPicker = forwardRef<HTMLElement, LxPickerProps>(({
  id,
  columns, mode, start, end, fields, value, onConfirm, onCancel, onScroll, placeholder = 'Please select',
  onChange, onNativeScroll, bindChange, bindScroll, catchChange, catchScroll,
  className, style, disabled, cancelText, cancelTextColor, cancelButtonColor,
  confirmText, confirmTextColor, confirmButtonColor, children,
  ...rest
}, ref) => {
  const [visible, setVisible] = useState(false);
  const reactId = useId();
  const pickerId = id ?? `lx-picker-${reactId.replace(/[:]/g, "")}`;
  const elementRef = useRef<HTMLElement | null>(null);
  const boundElementRef = useRef<HTMLElement | null>(null);
  const handlerRef = useRef({
    columns,
    mode,
    onConfirm,
    onCancel,
    onScroll,
    onChange,
    onNativeScroll,
  });
  handlerRef.current = {
    columns,
    mode,
    onConfirm,
    onCancel,
    onScroll,
    onChange,
    onNativeScroll,
  };
  const listenerMapRef = useRef<Record<string, EventListenerObject>>({
    change: {
      handleEvent: (event: Event) => {
        const detail = getCustomEventDetail<{
          confirmed?: boolean;
          cancelled?: boolean;
          value?: string | string[];
          index?: number | number[];
        }>(event);
        handlerRef.current.onChange?.(event as CustomEvent);
        if (detail.confirmed) {
          const nextValue =
            handlerRef.current.mode === 'date' || handlerRef.current.mode === 'time'
              ? (detail.value ?? '')
              : detail.index !== undefined
                ? getPickerValueFromIndex(handlerRef.current.columns, detail.index)
                : '';
          handlerRef.current.onConfirm?.(nextValue);
          setVisible(false);
          return;
        }
        if (detail.cancelled) {
          handlerRef.current.onCancel?.();
          setVisible(false);
        }
      },
    },
    scroll: {
      handleEvent: (event: Event) => {
        const detail = getCustomEventDetail<{
          value?: string | string[];
          index?: number | number[];
        }>(event);
        handlerRef.current.onNativeScroll?.(event as CustomEvent);
        if (detail.value !== undefined) {
          handlerRef.current.onScroll?.(detail.value);
          return;
        }
        if (detail.index !== undefined) {
          handlerRef.current.onScroll?.(getPickerValueFromIndex(handlerRef.current.columns, detail.index));
        }
      },
    },
  });
  const elementRefCallback = useCallback((element: HTMLElement | null) => {
    boundElementRef.current = bindElementEvents(boundElementRef.current, element, listenerMapRef.current);
    elementRef.current = element;
    assignForwardedRef(ref, element);
  }, [ref]);
  React.useEffect(() => () => {
    unbindElementEvents(boundElementRef.current, listenerMapRef.current);
    boundElementRef.current = null;
    elementRef.current = null;
  }, []);

  const handleClick = () => !disabled && setVisible(true);
  const pickerProps = buildPickerNativeAttrs({
    id: pickerId,
    columns,
    mode,
    start,
    end,
    fields,
    value,
    cancelText,
    cancelTextColor,
    cancelButtonColor,
    confirmText,
    confirmTextColor,
    confirmButtonColor,
    bindChange,
    bindScroll,
    catchChange,
    catchScroll,
  }, rest as Record<string, unknown>);
  const displayText = getPickerDisplayText(value, fields);

  return (
    <>
      {children ? (
        <div
          onClick={handleClick}
          className={className}
          style={{
            cursor: disabled ? 'not-allowed' : 'pointer',
            opacity: disabled ? 0.5 : 1,
            ...style,
          }}
        >
          {children}
        </div>
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
          <span style={{ color: value ? '#111' : '#9ca3af' }}>{displayText || placeholder}</span>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#9ca3af" strokeWidth="2">
            <path d="M6 9l6 6 6-6" />
          </svg>
        </div>
      )}
      {visible && React.createElement('lx-picker', { ref: elementRefCallback, ...pickerProps })}
    </>
  );
});

LxPicker.displayName = 'LxPicker';
