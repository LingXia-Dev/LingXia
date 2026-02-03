/**
 * Input APIs
 * Corresponds to: lingxia-logic/src/input.rs
 *
 * Platform support: Android only
 */

export interface KeyEvent {
  /** Key value following W3C naming (e.g. "Enter", "ArrowLeft", "a") */
  key: string;
  /** Physical key code (e.g. "ENTER", "DPAD_LEFT") */
  code: string;
  altKey?: boolean;
  ctrlKey?: boolean;
  shiftKey?: boolean;
  metaKey?: boolean;
  repeat?: boolean;
}

export type KeyEventCallback = (event: KeyEvent) => void;
