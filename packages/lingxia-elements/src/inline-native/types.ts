import type {
  CoreKind,
  HostFactoryKind,
  NativeErrorCode,
  PublicComponentName,
} from "./schema.js";

export interface RootRef {
  surfaceInstanceId: string;
  pageInstanceId: string;
  documentInstanceId: string;
  rootKey: string;
  rootEpoch: number;
}

export interface NodeRef extends RootRef {
  nodeKey: string;
  nodeEpoch: number;
}

export interface NativeError {
  code: NativeErrorCode;
  scope: "node" | "root";
  recoverable: boolean;
  root: RootRef;
  node?: NodeRef;
  message: string;
}

export interface PressPayload {
  source: "pointer" | "keyboard" | "accessibility";
}

export interface ValuePayload {
  value: number;
}

export interface FocusPayload {
  source: "pointer" | "keyboard" | "accessibility";
}

export interface PointerPayload {
  source: "pointer";
}

export type NativeHandler<TPayload> = (payload: TPayload) => void;

export type AuthorChild =
  | AuthorNode
  | string
  | number
  | false
  | null
  | undefined
  | readonly AuthorChild[];

export interface AuthorNode {
  type: string;
  authorId?: string;
  automationId?: string;
  props?: Record<string, unknown>;
  children?: AuthorChild;
  textContent?: string | number;
}

export interface CoreNode {
  kind: HostFactoryKind;
  authorType: PublicComponentName;
  authorId?: string;
  automationId?: string;
  props: Record<string, unknown>;
  children: CoreNode[];
  text?: string;
}

export interface CompiledNativeRoot {
  authorType: "LxNativeRoot";
  authorId?: string;
  automationId?: string;
  props: Record<string, unknown>;
  children: CoreNode[];
  kind: Extract<CoreKind, "root">;
}

export type CompileInlineNativeResult =
  | { ok: true; root: CompiledNativeRoot; diagnostics: NativeError[] }
  | { ok: false; error: NativeError; diagnostics: NativeError[] };

export interface CompileInlineNativeOptions {
  rootRef?: RootRef;
  /**
   * When true, `controls` defaults to `true` (author contract). Tests and
   * wrappers should leave this on; only a host fixture may override it.
   */
  defaultVideoControls?: boolean;
}

export const EMPTY_ROOT_REF: RootRef = {
  surfaceInstanceId: "",
  pageInstanceId: "",
  documentInstanceId: "",
  rootKey: "",
  rootEpoch: 0,
};
