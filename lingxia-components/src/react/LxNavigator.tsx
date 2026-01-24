import React from 'react';
import type {
  LxNavigatorAttributes,
  LxNavigatorEvent,
  NavigatorOpenType,
  NavigatorTarget
} from '../navigator.js';

// Import to ensure custom element is registered
import '../navigator.js';

export interface LxNavigatorProps extends Omit<LxNavigatorAttributes, 'onSuccess' | 'onFail' | 'onComplete'> {
  // Navigation
  url?: string;
  openType?: NavigatorOpenType;
  target?: NavigatorTarget; // Auto-inferred if not specified
  delta?: number;

  // Open external lxapp
  lxAppId?: string;
  path?: string; // Supports query string

  // Phone call
  phoneNumber?: string;

  // Hover effect
  hoverClass?: string;
  hoverStopPropagation?: boolean;
  hoverStartTime?: number;
  hoverStayTime?: number;

  // Styling
  className?: string;
  style?: React.CSSProperties;

  // Events
  onSuccess?: (e: LxNavigatorEvent) => void;
  onFail?: (e: LxNavigatorEvent) => void;
  onComplete?: (e: LxNavigatorEvent) => void;

  // Children
  children?: React.ReactNode;
}

export const LxNavigator = React.forwardRef<HTMLElement, LxNavigatorProps>(
  (props, ref) => {
    const {
      url,
      openType = 'navigate',
      target, // Auto-inferred, no default
      delta = 1,
      lxAppId,
      path,
      phoneNumber,
      hoverClass = 'navigator-hover',
      hoverStopPropagation = false,
      hoverStartTime = 20,
      hoverStayTime = 70,
      className,
      style,
      onSuccess,
      onFail,
      onComplete,
      children,
      ...rest
    } = props;

    const elementRef = React.useRef<HTMLElement>(null);

    React.useImperativeHandle(ref, () => elementRef.current!);

    React.useEffect(() => {
      const element = elementRef.current;
      if (!element) return;

      const handleSuccess = (e: Event) => {
        if (onSuccess) {
          onSuccess(e as LxNavigatorEvent);
        }
      };

      const handleFail = (e: Event) => {
        if (onFail) {
          onFail(e as LxNavigatorEvent);
        }
      };

      const handleComplete = (e: Event) => {
        if (onComplete) {
          onComplete(e as LxNavigatorEvent);
        }
      };

      element.addEventListener('success', handleSuccess);
      element.addEventListener('fail', handleFail);
      element.addEventListener('complete', handleComplete);

      return () => {
        element.removeEventListener('success', handleSuccess);
        element.removeEventListener('fail', handleFail);
        element.removeEventListener('complete', handleComplete);
      };
    }, [onSuccess, onFail, onComplete]);

    return React.createElement(
      'lx-navigator',
      {
        ref: elementRef,
        url,
        'open-type': openType,
        target, // Optional, will be auto-inferred if not specified
        delta: delta.toString(),
        'lx-app-id': lxAppId,
        path,
        'phone-number': phoneNumber,
        'hover-class': hoverClass,
        'hover-stop-propagation': hoverStopPropagation.toString(),
        'hover-start-time': hoverStartTime.toString(),
        'hover-stay-time': hoverStayTime.toString(),
        className,
        style,
        ...rest
      },
      children
    );
  }
);

LxNavigator.displayName = 'LxNavigator';
