import { appendDataAttrs } from "./native_component_wrapper_shared.js";

export interface InputNativeAttrOptions {
  id?: string;
  modelValue?: string;
  value?: string;
  defaultValue?: string;
  type?: "text" | "number" | "password" | "digit";
  password?: boolean;
  placeholder?: string;
  placeholderStyle?: string;
  placeholderClass?: string;
  maxlength?: number;
  cursorSpacing?: number;
  autoFocus?: boolean;
  disabled?: boolean;
  focus?: boolean;
  confirmType?: "send" | "search" | "next" | "go" | "done";
  alwaysEmbed?: boolean;
  confirmHold?: boolean;
  cursor?: number;
  cursorColor?: string;
  selectionStart?: number;
  selectionEnd?: number;
  adjustPosition?: boolean;
  holdKeyboard?: boolean;
}

export interface TextareaNativeAttrOptions {
  id?: string;
  modelValue?: string;
  value?: string;
  defaultValue?: string;
  placeholder?: string;
  placeholderStyle?: string;
  placeholderClass?: string;
  maxlength?: number;
  disabled?: boolean;
  autoFocus?: boolean;
  focus?: boolean;
  autoHeight?: boolean;
  cursorSpacing?: number;
  showConfirmBar?: boolean;
  adjustPosition?: boolean;
  holdKeyboard?: boolean;
  disableDefaultPadding?: boolean;
  confirmType?: "send" | "search" | "next" | "go" | "done" | "return";
  confirmHold?: boolean;
  fixed?: boolean;
  adjustKeyboardTo?: "cursor" | "bottom";
  cursor?: number;
  selectionStart?: number;
  selectionEnd?: number;
}

export function buildInputNativeAttrs(
  options: InputNativeAttrOptions,
  extraAttrs: Record<string, unknown> = {},
  mounted = false
): Record<string, string> {
  const result: Record<string, string> = {};
  const explicitId = typeof options.id === "string" ? options.id.trim() : "";

  if (explicitId.length > 0) result.id = explicitId;

  const nextValue = options.modelValue ?? options.value;
  if (nextValue !== undefined) {
    result.value = nextValue;
  } else if (!mounted && options.defaultValue !== undefined) {
    result.value = options.defaultValue;
  }

  if (options.type) result.type = options.type;
  if (options.password) result.password = "true";
  if (options.placeholder) result.placeholder = options.placeholder;
  if (options.placeholderStyle) result["placeholder-style"] = options.placeholderStyle;
  if (options.placeholderClass) result["placeholder-class"] = options.placeholderClass;
  if (options.maxlength !== undefined) result.maxlength = String(options.maxlength);
  if (options.cursorSpacing !== undefined) result["cursor-spacing"] = String(options.cursorSpacing);
  if (options.autoFocus) result["auto-focus"] = "true";
  if (options.disabled) result.disabled = "true";
  if (options.focus !== undefined) result.focus = options.focus ? "true" : "false";
  if (options.confirmType) result["confirm-type"] = options.confirmType;
  if (options.alwaysEmbed) result["always-embed"] = "true";
  if (options.confirmHold) result["confirm-hold"] = "true";
  if (options.cursor !== undefined) result.cursor = String(options.cursor);
  if (options.cursorColor) result["cursor-color"] = options.cursorColor;
  if (options.selectionStart !== undefined) result["selection-start"] = String(options.selectionStart);
  if (options.selectionEnd !== undefined) result["selection-end"] = String(options.selectionEnd);
  if (options.adjustPosition === false) result["adjust-position"] = "false";
  if (options.holdKeyboard) result["hold-keyboard"] = "true";

  appendDataAttrs(extraAttrs, result);
  return result;
}

export function buildTextareaNativeAttrs(
  options: TextareaNativeAttrOptions,
  extraAttrs: Record<string, unknown> = {},
  mounted = false
): Record<string, string> {
  const result: Record<string, string> = {};
  const explicitId = typeof options.id === "string" ? options.id.trim() : "";

  if (explicitId.length > 0) result.id = explicitId;

  const nextValue = options.modelValue ?? options.value;
  if (nextValue !== undefined) {
    result.value = nextValue;
  } else if (!mounted && options.defaultValue !== undefined) {
    result.value = options.defaultValue;
  }

  if (options.placeholder) result.placeholder = options.placeholder;
  if (options.placeholderStyle) result["placeholder-style"] = options.placeholderStyle;
  if (options.placeholderClass) result["placeholder-class"] = options.placeholderClass;
  if (options.maxlength !== undefined) result.maxlength = String(options.maxlength);
  if (options.disabled) result.disabled = "true";
  if (options.autoFocus) result["auto-focus"] = "true";
  if (options.focus !== undefined) result.focus = options.focus ? "true" : "false";
  if (options.autoHeight) result["auto-height"] = "true";
  if (options.cursorSpacing !== undefined) result["cursor-spacing"] = String(options.cursorSpacing);
  if (options.showConfirmBar === false) result["show-confirm-bar"] = "false";
  if (options.adjustPosition === false) result["adjust-position"] = "false";
  if (options.holdKeyboard) result["hold-keyboard"] = "true";
  if (options.disableDefaultPadding) result["disable-default-padding"] = "true";
  if (options.confirmType) result["confirm-type"] = options.confirmType;
  if (options.confirmHold) result["confirm-hold"] = "true";
  if (options.fixed) result.fixed = "true";
  if (options.adjustKeyboardTo) result["adjust-keyboard-to"] = options.adjustKeyboardTo;
  if (options.cursor !== undefined) result.cursor = String(options.cursor);
  if (options.selectionStart !== undefined) result["selection-start"] = String(options.selectionStart);
  if (options.selectionEnd !== undefined) result["selection-end"] = String(options.selectionEnd);

  appendDataAttrs(extraAttrs, result);
  return result;
}
