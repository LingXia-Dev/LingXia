import { appendBindingAndDatasetAttrs } from "./native_component_wrapper_shared.js";

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
  bindInput?: string;
  bindChange?: string;
  bindFocus?: string;
  bindBlur?: string;
  bindConfirm?: string;
  bindKeyboardHeightChange?: string;
  bindNicknameReview?: string;
  catchInput?: string;
  catchChange?: string;
  catchFocus?: string;
  catchBlur?: string;
  catchConfirm?: string;
  catchKeyboardHeightChange?: string;
  catchNicknameReview?: string;
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
  bindInput?: string;
  bindChange?: string;
  bindFocus?: string;
  bindBlur?: string;
  bindConfirm?: string;
  bindLineChange?: string;
  bindKeyboardHeightChange?: string;
  catchInput?: string;
  catchChange?: string;
  catchFocus?: string;
  catchBlur?: string;
  catchConfirm?: string;
  catchLineChange?: string;
  catchKeyboardHeightChange?: string;
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

  if (options.bindInput) result.bindinput = options.bindInput;
  if (options.bindChange) result.bindchange = options.bindChange;
  if (options.bindFocus) result.bindfocus = options.bindFocus;
  if (options.bindBlur) result.bindblur = options.bindBlur;
  if (options.bindConfirm) result.bindconfirm = options.bindConfirm;
  if (options.bindKeyboardHeightChange) result.bindkeyboardheightchange = options.bindKeyboardHeightChange;
  if (options.bindNicknameReview) result.bindnicknamereview = options.bindNicknameReview;
  if (options.catchInput) result.catchinput = options.catchInput;
  if (options.catchChange) result.catchchange = options.catchChange;
  if (options.catchFocus) result.catchfocus = options.catchFocus;
  if (options.catchBlur) result.catchblur = options.catchBlur;
  if (options.catchConfirm) result.catchconfirm = options.catchConfirm;
  if (options.catchKeyboardHeightChange) result.catchkeyboardheightchange = options.catchKeyboardHeightChange;
  if (options.catchNicknameReview) result.catchnicknamereview = options.catchNicknameReview;

  appendBindingAndDatasetAttrs(extraAttrs, result);
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

  if (options.bindInput) result.bindinput = options.bindInput;
  if (options.bindChange) result.bindchange = options.bindChange;
  if (options.bindFocus) result.bindfocus = options.bindFocus;
  if (options.bindBlur) result.bindblur = options.bindBlur;
  if (options.bindConfirm) result.bindconfirm = options.bindConfirm;
  if (options.bindLineChange) result.bindlinechange = options.bindLineChange;
  if (options.bindKeyboardHeightChange) result.bindkeyboardheightchange = options.bindKeyboardHeightChange;
  if (options.catchInput) result.catchinput = options.catchInput;
  if (options.catchChange) result.catchchange = options.catchChange;
  if (options.catchFocus) result.catchfocus = options.catchFocus;
  if (options.catchBlur) result.catchblur = options.catchBlur;
  if (options.catchConfirm) result.catchconfirm = options.catchConfirm;
  if (options.catchLineChange) result.catchlinechange = options.catchLineChange;
  if (options.catchKeyboardHeightChange) result.catchkeyboardheightchange = options.catchKeyboardHeightChange;

  appendBindingAndDatasetAttrs(extraAttrs, result);
  return result;
}
