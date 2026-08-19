import { nativeError } from "./errors.js";
import {
  CONFLICTING_CONTROL_ICONS,
  NATIVE_ACTION_ICONS,
  PUBLIC_COMPONENT_NAMES,
  TAG_TO_AUTHOR_COMPONENT,
  type PublicComponentName,
} from "./schema.js";
import {
  EMPTY_ROOT_REF,
  type AuthorChild,
  type AuthorNode,
  type CompileInlineNativeOptions,
  type CompileInlineNativeResult,
  type CompiledNativeRoot,
  type CoreNode,
  type NativeError,
  type RootRef,
} from "./types.js";

const PUBLIC_NAME_SET = new Set<string>(PUBLIC_COMPONENT_NAMES);
const CONFLICTING_ICON_SET = new Set<string>(CONFLICTING_CONTROL_ICONS);
const ACTION_ICON_SET = new Set<string>(NATIVE_ACTION_ICONS);

/**
 * Compile one author Root into the host core tree. Cover/Button expand here;
 * illegal children never become a host commit.
 */
export function compileInlineNativeRoot(
  author: AuthorNode,
  options: CompileInlineNativeOptions = {}
): CompileInlineNativeResult {
  const rootRef = options.rootRef ?? EMPTY_ROOT_REF;
  const diagnostics: NativeError[] = [];
  const authorType = normalizeAuthorType(author.type);

  if (authorType !== "LxNativeRoot") {
    return fail(
      nativeError(
        "NATIVE_ROOT_INVALID_STRUCTURE",
        authorType === "LxVideo"
          ? "LxVideo must be a direct child of an explicit LxNativeRoot; implicit roots are not created"
          : `inline native trees must start at LxNativeRoot, got ${author.type || "<unknown>"}`,
        { root: rootRef }
      ),
      diagnostics
    );
  }

  const flattened = flattenAuthorChildren(author.children);
  const textChild = flattened.find((child) => child.kind === "text");
  if (textChild) {
    return fail(
      nativeError(
        "NATIVE_ROOT_INVALID_STRUCTURE",
        "LxNativeRoot cannot contain bare text; wrap copy in LxNativeText",
        { root: rootRef }
      ),
      diagnostics
    );
  }

  const authorChildren: AuthorNode[] = [];
  for (const child of flattened) {
    if (child.kind === "text") {
      continue;
    }
    if (child.kind === "unknown") {
      return fail(
        nativeError(
          "NATIVE_ROOT_INVALID_STRUCTURE",
          `LxNativeRoot cannot contain DOM or unregistered children (${child.label})`,
          { root: rootRef }
        ),
        diagnostics
      );
    }
    authorChildren.push(child.node);
  }

  const props = { ...(author.props ?? {}) };
  const defaultVideoControls = options.defaultVideoControls !== false;
  const structureError = validateRootAuthorChildren(
    authorChildren,
    rootRef,
    defaultVideoControls
  );
  if (structureError) {
    return fail(structureError, diagnostics);
  }

  const expanded: CoreNode[] = [];
  for (const child of authorChildren) {
    const result = expandAuthorNode(child, rootRef, {
      allowVideo: true,
      parent: "LxNativeRoot",
      defaultVideoControls,
    });
    if (!result.ok) {
      return fail(result.error, diagnostics);
    }
    expanded.push(result.node);
    diagnostics.push(...result.diagnostics);
  }

  const root: CompiledNativeRoot = {
    kind: "root",
    authorType: "LxNativeRoot",
    authorId: author.authorId,
    automationId: author.automationId,
    props,
    children: expanded,
  };
  return { ok: true, root, diagnostics };
}

/**
 * Scan a page-level author forest. A bare LxVideo (or a Video nested in a
 * non-Root) is `NATIVE_ROOT_INVALID_STRUCTURE`.
 */
export function compileInlineNativeForest(
  nodes: readonly AuthorNode[],
  options: CompileInlineNativeOptions = {}
): CompileInlineNativeResult | { ok: true; roots: CompiledNativeRoot[]; diagnostics: NativeError[] } {
  const roots: CompiledNativeRoot[] = [];
  const diagnostics: NativeError[] = [];
  for (const node of nodes) {
    const type = normalizeAuthorType(node.type);
    if (type === "LxVideo") {
      return {
        ok: false,
        error: nativeError(
          "NATIVE_ROOT_INVALID_STRUCTURE",
          "LxVideo must be a direct child of an explicit LxNativeRoot; implicit roots are not created",
          { root: options.rootRef ?? EMPTY_ROOT_REF }
        ),
        diagnostics,
      };
    }
    if (type === "LxNativeRoot") {
      const compiled = compileInlineNativeRoot(node, options);
      if (!compiled.ok) {
        return compiled;
      }
      roots.push(compiled.root);
      diagnostics.push(...compiled.diagnostics);
      continue;
    }
    if (type) {
      const nestedVideo = findFirstVideo(node);
      if (nestedVideo) {
        return {
          ok: false,
          error: nativeError(
            "NATIVE_ROOT_INVALID_STRUCTURE",
            "LxVideo must be a direct child of an explicit LxNativeRoot; it cannot live in View/Cover or outside a Root",
            { root: options.rootRef ?? EMPTY_ROOT_REF }
          ),
          diagnostics,
        };
      }
    }
  }
  return { ok: true, roots, diagnostics };
}

function fail(
  error: NativeError,
  diagnostics: NativeError[]
): CompileInlineNativeResult {
  return { ok: false, error, diagnostics };
}

type FlatChild =
  | { kind: "node"; node: AuthorNode }
  | { kind: "text"; value: string }
  | { kind: "unknown"; label: string };

function flattenAuthorChildren(children: AuthorChild | undefined): FlatChild[] {
  const out: FlatChild[] = [];
  walk(children);
  return out;

  function walk(value: AuthorChild | undefined): void {
    if (value === false || value === null || value === undefined) {
      return;
    }
    if (Array.isArray(value)) {
      for (const entry of value) {
        walk(entry);
      }
      return;
    }
    if (typeof value === "string" || typeof value === "number") {
      out.push({ kind: "text", value: String(value) });
      return;
    }
    if (typeof value === "object" && value && typeof (value as AuthorNode).type === "string") {
      const node = value as AuthorNode;
      if (node.type === "#text") {
        const text = node.textContent == null ? "" : String(node.textContent);
        if (text) {
          out.push({ kind: "text", value: text });
        }
        return;
      }
      out.push({ kind: "node", node });
      return;
    }
    out.push({ kind: "unknown", label: describeUnknown(value) });
  }
}

function describeUnknown(value: unknown): string {
  if (value && typeof value === "object") {
    const record = value as { type?: unknown; tagName?: unknown };
    if (typeof record.type === "string") return record.type;
    if (typeof record.tagName === "string") return record.tagName;
    return Object.prototype.toString.call(value);
  }
  return String(value);
}

export function normalizeAuthorType(type: string | undefined): PublicComponentName | undefined {
  if (!type) return undefined;
  const trimmed = type.trim();
  if (PUBLIC_NAME_SET.has(trimmed)) {
    return trimmed as PublicComponentName;
  }
  const lower = trimmed.toLowerCase();
  if (lower in TAG_TO_AUTHOR_COMPONENT) {
    return TAG_TO_AUTHOR_COMPONENT[lower as keyof typeof TAG_TO_AUTHOR_COMPONENT];
  }
  return undefined;
}

function validateRootAuthorChildren(
  children: readonly AuthorNode[],
  rootRef: RootRef,
  defaultVideoControls: boolean
): NativeError | undefined {
  let hasControlsVideo = false;
  let hasSlider = false;
  let conflictingButton: string | undefined;

  for (const child of children) {
    const type = normalizeAuthorType(child.type);
    if (type === "LxNativeRoot") {
      return nativeError(
        "NATIVE_ROOT_INVALID_STRUCTURE",
        "LxNativeRoot cannot nest another LxNativeRoot",
        { root: rootRef }
      );
    }
    if (type === "LxVideo" && videoControlsEnabled(child.props, defaultVideoControls)) {
      hasControlsVideo = true;
    }
  }

  walkForConflicts(children);

  if (hasControlsVideo && hasSlider) {
    return nativeError(
      "NATIVE_ROOT_INVALID_STRUCTURE",
      "controls={true} cannot share a Root with LxNativeSlider; set controls={false} or drop the Slider",
      { root: rootRef }
    );
  }
  if (hasControlsVideo && conflictingButton) {
    return nativeError(
      "NATIVE_ROOT_INVALID_STRUCTURE",
      `controls={true} cannot share a Root with a ${conflictingButton} Button; set controls={false} or use a non-chrome icon`,
      { root: rootRef }
    );
  }
  return undefined;

  function walkForConflicts(nodes: readonly AuthorNode[]): void {
    for (const node of nodes) {
      const type = normalizeAuthorType(node.type);
      if (type === "LxNativeSlider") {
        hasSlider = true;
      }
      if (type === "LxNativeButton") {
        const icon = readButtonIcon(node.props);
        if (icon && CONFLICTING_ICON_SET.has(icon)) {
          conflictingButton = icon;
        }
      }
      const nested = flattenAuthorChildren(node.children)
        .filter((entry): entry is { kind: "node"; node: AuthorNode } => entry.kind === "node")
        .map((entry) => entry.node);
      if (nested.length > 0) {
        walkForConflicts(nested);
      }
    }
  }
}

function videoControlsEnabled(
  props: Record<string, unknown> | undefined,
  defaultVideoControls: boolean
): boolean {
  if (!props || !("controls" in props) || props.controls === undefined || props.controls === null) {
    return defaultVideoControls;
  }
  return parseBooleanAttr(props.controls, defaultVideoControls);
}

export function parseBooleanAttr(value: unknown, defaultValue = false): boolean {
  if (value === true || value === "") return true;
  if (value === false) return false;
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase();
    if (normalized === "true" || normalized === "") return true;
    if (normalized === "false") return false;
  }
  if (typeof value === "number") {
    return value !== 0;
  }
  return defaultValue;
}

function readButtonIcon(props: Record<string, unknown> | undefined): string | undefined {
  const icon = props?.icon;
  if (typeof icon === "string" && icon.trim()) {
    return icon.trim();
  }
  return undefined;
}

interface ExpandContext {
  allowVideo: boolean;
  parent: PublicComponentName;
  defaultVideoControls: boolean;
}

type ExpandResult =
  | { ok: true; node: CoreNode; diagnostics: NativeError[] }
  | { ok: false; error: NativeError; diagnostics: NativeError[] };

function expandAuthorNode(
  author: AuthorNode,
  rootRef: RootRef,
  context: ExpandContext
): ExpandResult {
  const type = normalizeAuthorType(author.type);
  if (!type) {
    return {
      ok: false,
      error: nativeError(
        "NATIVE_ROOT_INVALID_STRUCTURE",
        `LxNativeRoot cannot contain DOM or unregistered children (${author.type || "<unknown>"})`,
        { root: rootRef }
      ),
      diagnostics: [],
    };
  }

  if (type === "LxNativeRoot") {
    return {
      ok: false,
      error: nativeError(
        "NATIVE_ROOT_INVALID_STRUCTURE",
        "LxNativeRoot cannot nest another LxNativeRoot",
        { root: rootRef }
      ),
      diagnostics: [],
    };
  }

  if (type === "LxVideo") {
    if (!context.allowVideo || context.parent !== "LxNativeRoot") {
      return {
        ok: false,
        error: nativeError(
          "NATIVE_ROOT_INVALID_STRUCTURE",
          "LxVideo must be a direct child of LxNativeRoot; View/Cover cannot wrap it",
          { root: rootRef }
        ),
        diagnostics: [],
      };
    }
    const nested = flattenAuthorChildren(author.children);
    if (nested.length > 0) {
      return {
        ok: false,
        error: nativeError(
          "NATIVE_ROOT_INVALID_STRUCTURE",
          "LxVideo cannot have children",
          { root: rootRef }
        ),
        diagnostics: [],
      };
    }
    const props = { ...(author.props ?? {}) };
    if (!("controls" in props) || props.controls === undefined) {
      props.controls = context.defaultVideoControls;
    } else {
      props.controls = parseBooleanAttr(props.controls, context.defaultVideoControls);
    }
    const pressHandler = props.onPress;
    if (
      props.controls === false &&
      typeof pressHandler === "function" &&
      !nonEmptyAriaLabel(props)
    ) {
      return {
        ok: false,
        error: nativeError(
          "NATIVE_COMPONENT_INVALID_PROPS",
          "LxVideo with controls={false} and onPress requires a non-empty aria-label",
          { root: rootRef, scope: "node", recoverable: true }
        ),
        diagnostics: [],
      };
    }
    return {
      ok: true,
      node: {
        kind: "video",
        authorType: "LxVideo",
        authorId: author.authorId,
        automationId: author.automationId,
        props,
        children: [],
      },
      diagnostics: [],
    };
  }

  if (type === "LxNativeText") {
    const text = readTextContent(author);
    return {
      ok: true,
      node: {
        kind: "text",
        authorType: "LxNativeText",
        authorId: author.authorId,
        automationId: author.automationId,
        props: { ...(author.props ?? {}), pointerEvents: author.props?.pointerEvents ?? "none" },
        children: [],
        text,
      },
      diagnostics: [],
    };
  }

  if (type === "LxNativeSlider") {
    const nested = flattenAuthorChildren(author.children);
    if (nested.length > 0) {
      return {
        ok: false,
        error: nativeError(
          "NATIVE_ROOT_INVALID_STRUCTURE",
          "LxNativeSlider cannot have children",
          { root: rootRef }
        ),
        diagnostics: [],
      };
    }
    if (!nonEmptyAriaLabel(author.props)) {
      return {
        ok: false,
        error: nativeError(
          "NATIVE_COMPONENT_INVALID_PROPS",
          "LxNativeSlider requires a non-empty aria-label",
          { root: rootRef, scope: "node", recoverable: true }
        ),
        diagnostics: [],
      };
    }
    return {
      ok: true,
      node: {
        kind: "slider",
        authorType: "LxNativeSlider",
        authorId: author.authorId,
        automationId: author.automationId,
        props: normalizeSliderProps(author.props),
        children: [],
      },
      diagnostics: [],
    };
  }

  if (type === "LxNativeButton") {
    const nested = flattenAuthorChildren(author.children);
    if (nested.some((entry) => entry.kind === "node")) {
      return {
        ok: false,
        error: nativeError(
          "NATIVE_ROOT_INVALID_STRUCTURE",
          "LxNativeButton cannot wrap interactive or native children; use label/icon",
          { root: rootRef }
        ),
        diagnostics: [],
      };
    }
    const label = readButtonLabel(author, nested);
    const icon = author.props?.icon;
    if (!label && !icon && !nonEmptyAriaLabel(author.props)) {
      return {
        ok: false,
        error: nativeError(
          "NATIVE_COMPONENT_INVALID_PROPS",
          "LxNativeButton without label must provide a non-empty aria-label",
          { root: rootRef, scope: "node", recoverable: true }
        ),
        diagnostics: [],
      };
    }
    if (typeof icon === "string" && icon && !ACTION_ICON_SET.has(icon) && !isResourceIcon(icon)) {
      return {
        ok: false,
        error: nativeError(
          "NATIVE_COMPONENT_INVALID_PROPS",
          `LxNativeButton icon "${icon}" is not a NativeActionIcon or { resource }`,
          { root: rootRef, scope: "node", recoverable: true }
        ),
        diagnostics: [],
      };
    }
    return {
      ok: true,
      node: {
        kind: "tappable",
        authorType: "LxNativeButton",
        authorId: author.authorId,
        automationId: author.automationId,
        props: {
          ...normalizeButtonProps(author.props),
          content: buildButtonContent(label, icon),
        },
        children: [],
      },
      diagnostics: [],
    };
  }

  if (type === "LxNativeCover" || type === "LxNativeView") {
    const childContext: ExpandContext = {
      allowVideo: false,
      parent: type,
      defaultVideoControls: context.defaultVideoControls,
    };
    const expandedChildren: CoreNode[] = [];
    const diagnostics: NativeError[] = [];
    const flattened = flattenAuthorChildren(author.children);
    for (const child of flattened) {
      if (child.kind === "text") {
        return {
          ok: false,
          error: nativeError(
            "NATIVE_ROOT_INVALID_STRUCTURE",
            `${type} cannot contain bare text; wrap copy in LxNativeText`,
            { root: rootRef }
          ),
          diagnostics,
        };
      }
      if (child.kind === "unknown") {
        return {
          ok: false,
          error: nativeError(
            "NATIVE_ROOT_INVALID_STRUCTURE",
            `${type} cannot contain DOM or unregistered children (${child.label})`,
            { root: rootRef }
          ),
          diagnostics,
        };
      }
      const result = expandAuthorNode(child.node, rootRef, childContext);
      if (!result.ok) {
        return result;
      }
      expandedChildren.push(result.node);
      diagnostics.push(...result.diagnostics);
    }

    if (type === "LxNativeCover") {
      return {
        ok: true,
        node: {
          kind: "view",
          authorType: "LxNativeCover",
          authorId: author.authorId,
          automationId: author.automationId,
          props: normalizeCoverProps(author.props),
          children: expandedChildren,
        },
        diagnostics,
      };
    }

    return {
      ok: true,
      node: {
        kind: "view",
        authorType: "LxNativeView",
        authorId: author.authorId,
        automationId: author.automationId,
        props: {
          pointerEvents: author.props?.pointerEvents ?? "auto",
          ...(author.props ?? {}),
        },
        children: expandedChildren,
      },
      diagnostics,
    };
  }

  return {
    ok: false,
    error: nativeError(
      "NATIVE_ROOT_INVALID_STRUCTURE",
      `unsupported inline native component ${author.type}`,
      { root: rootRef }
    ),
    diagnostics: [],
  };
}

function findFirstVideo(node: AuthorNode): AuthorNode | undefined {
  if (normalizeAuthorType(node.type) === "LxVideo") {
    return node;
  }
  for (const child of flattenAuthorChildren(node.children)) {
    if (child.kind === "node") {
      const found = findFirstVideo(child.node);
      if (found) return found;
    }
  }
  return undefined;
}

function readTextContent(author: AuthorNode): string {
  if (author.textContent !== undefined && author.textContent !== null) {
    return String(author.textContent);
  }
  const parts: string[] = [];
  for (const child of flattenAuthorChildren(author.children)) {
    if (child.kind === "text") {
      parts.push(child.value);
    }
  }
  return parts.join("");
}

function readButtonLabel(author: AuthorNode, nested: FlatChild[]): string {
  const labeled = author.props?.label;
  if (typeof labeled === "string" && labeled.trim()) {
    return labeled;
  }
  return nested
    .filter((entry): entry is { kind: "text"; value: string } => entry.kind === "text")
    .map((entry) => entry.value)
    .join("");
}

function nonEmptyAriaLabel(props: Record<string, unknown> | undefined): boolean {
  if (!props) return false;
  const label = props["aria-label"] ?? props.ariaLabel;
  return typeof label === "string" && label.trim().length > 0;
}

function isResourceIcon(icon: unknown): boolean {
  return !!icon && typeof icon === "object" && icon !== null && "resource" in (icon as object);
}

function buildButtonContent(label: string, icon: unknown): Record<string, unknown> {
  const content: Record<string, unknown> = {};
  if (label) content.text = label;
  if (typeof icon === "string" && icon) {
    content.icon = { kind: "semantic", name: icon };
  } else if (isResourceIcon(icon)) {
    content.icon = { kind: "resource", resource: (icon as { resource: unknown }).resource };
  }
  return content;
}

function normalizeCoverProps(props: Record<string, unknown> | undefined): Record<string, unknown> {
  const next = { ...(props ?? {}) };
  if (next.pointerEvents === undefined) {
    next.pointerEvents = "box-none";
  }
  const scrim = typeof next.scrim === "string" ? next.scrim : "none";
  const scrimOpacity =
    typeof next.scrimOpacity === "number"
      ? next.scrimOpacity
      : typeof next.scrimOpacity === "string"
        ? Number(next.scrimOpacity)
        : 0.6;
  next.scrimPaint = {
    scrim,
    opacity: Number.isFinite(scrimOpacity) ? scrimOpacity : 0.6,
  };
  next.coverPreset = { position: "absolute", inset: 0 };
  return next;
}

function normalizeButtonProps(props: Record<string, unknown> | undefined): Record<string, unknown> {
  const next = { ...(props ?? {}) };
  if (next.intent === undefined) next.intent = "neutral";
  if (next.emphasis === undefined) next.emphasis = "secondary";
  if (next.size === undefined) next.size = "regular";
  if (next.iconPosition === undefined) next.iconPosition = "start";
  if (next.pointerEvents === undefined) next.pointerEvents = "auto";
  return next;
}

function normalizeSliderProps(props: Record<string, unknown> | undefined): Record<string, unknown> {
  const next = { ...(props ?? {}) };
  if (next.min === undefined) next.min = 0;
  if (next.max === undefined) next.max = 100;
  if (next.step === undefined) next.step = 0;
  if (next.valueLabel === undefined) next.valueLabel = "none";
  if (next.pointerEvents === undefined) next.pointerEvents = "auto";
  return next;
}

/** Collect author nodes from a Root element's light DOM (skips fallback). */
export function collectAuthorTreeFromElement(root: Element): AuthorNode {
  return readElementAsAuthor(root);
}

function readElementAsAuthor(element: Element): AuthorNode {
  const tag = element.tagName.toLowerCase();
  const type = normalizeAuthorType(tag) ?? tag;
  const props = readElementProps(element);
  const authorId = element.getAttribute("id") ?? undefined;
  const automationId = element.getAttribute("automation-id") ?? undefined;
  if (type === "LxNativeText") {
    return {
      type,
      authorId,
      automationId,
      props,
      textContent: element.textContent ?? "",
    };
  }
  const children: AuthorNode[] = [];
  for (const child of Array.from(element.childNodes)) {
    if (child.nodeType === 3) {
      const text = (child.textContent ?? "").trim();
      if (text) {
        children.push({ type: "#text", textContent: text, props: {} });
      }
      continue;
    }
    if (child.nodeType !== 1) continue;
    const el = child as Element;
    if (isFallbackElement(el)) continue;
    children.push(readElementAsAuthor(el));
  }
  return { type, authorId, automationId, props, children };
}

export function findInlineNativeRoot(element: Element): Element | null {
  if (typeof element.closest === "function") {
    return element.closest("lx-native-root");
  }
  let current: Element | null = element.parentElement;
  while (current) {
    if (current.tagName.toLowerCase() === "lx-native-root") {
      return current;
    }
    current = current.parentElement;
  }
  return null;
}

export function isFallbackElement(element: Element): boolean {
  if (element.hasAttribute("data-lx-native-fallback")) {
    return true;
  }
  const tag = element.tagName.toLowerCase();
  if (tag === "template" && element.getAttribute("slot") === "fallback") {
    return true;
  }
  return false;
}

function readElementProps(element: Element): Record<string, unknown> {
  const props: Record<string, unknown> = {};
  for (const attr of Array.from(element.attributes)) {
    const name = attr.name;
    if (name === "id" || name === "automation-id" || name === "class" || name === "style") {
      continue;
    }
    props[camelize(name)] = attr.value;
    if (name.startsWith("aria-")) {
      props[name] = attr.value;
    }
  }
  const anyEl = element as HTMLElement & Record<string, unknown>;
  const propertyKeys = [
    "src",
    "poster",
    "controls",
    "scrim",
    "scrimOpacity",
    "icon",
    "label",
    "value",
    "min",
    "max",
    "step",
    "bufferedValue",
    "valueLabel",
    "pointerEvents",
    "hidden",
    "hiddenTransition",
    "fullscreenScope",
    "onPress",
    "watermark",
    "qualities",
    "quality",
    "playbackRates",
    "rate",
    "objectFit",
    "contentRotate",
    "live",
    "autoplay",
    "loop",
    "muted",
    "volume",
    "progressBar",
    "intent",
    "emphasis",
    "size",
    "hitSlop",
    "disabled",
    "pressed",
    "expanded",
    "loading",
    "maxLines",
    "fontSize",
    "fontWeight",
    "lineHeight",
    "textAlign",
    "color",
    "dir",
    "role",
  ];
  for (const key of propertyKeys) {
    if (key in anyEl && anyEl[key] !== undefined && anyEl[key] !== null) {
      props[key] = anyEl[key];
    }
  }
  const ariaLabel = element.getAttribute("aria-label");
  if (ariaLabel) {
    props["aria-label"] = ariaLabel;
    props.ariaLabel = ariaLabel;
  }
  return props;
}

function camelize(name: string): string {
  return name.replace(/-([a-z])/g, (_, ch: string) => ch.toUpperCase());
}
