import type { AbortSignalLike } from './signal';

// The lxapp agent API's types, here until `lx.agent` ships them.

export type AgentEffect = 'read' | 'write' | 'sensitive';

export interface AgentTool<Args = Record<string, unknown>> {
  /** `^[a-z][a-z0-9_]{0,63}$`, unique within the lxapp. */
  readonly name: string;
  /** One line, shown in the list every conversation sees. */
  readonly summary: string;
  /** Full instructions for the model, fetched on demand (`info`). */
  readonly description: string;
  /** JSON Schema of `args`; the host validates against it before anything runs. */
  readonly inputSchema: Readonly<Record<string, unknown>>;
  /**
   * What a call does. `read`: changes nothing the person has not approved. `write`: changes the
   * person's data. `sensitive`: reveals or sends something private, or cannot be undone. A function
   * decides per call, before running, and names the change for the consent card.
   */
  readonly effect: AgentEffect | ((args: Args, call: AgentCall) => AgentAssessment | Promise<AgentAssessment>);
  run(args: Args, call: AgentCall): Promise<AgentResult>;
}

export interface AgentAssessment {
  readonly effect: AgentEffect;
  /** The change in the caller's language, e.g. 《第三周》: replace 2 blocks. Shown on the consent card. */
  readonly summary: string;
}

/** Filled in by the host from the sealed calling session; nothing here comes from the arguments. */
export interface AgentCall {
  readonly caller: { readonly appId: string; readonly name: string };
  /** The caller's conversation, stable for its lifetime. */
  readonly conversation: string;
  /** The agent's name as the person knows it (Codex, Kimi). */
  readonly agent: string;
  /** The person approved this call (always true for `read`). */
  readonly approved: boolean;
  /** BCP 47; word `summary` and errors in it. */
  readonly language: string;
  /** Aborted on timeout, conversation end or the caller's Stop. */
  readonly signal: AbortSignalLike;
}

export type AgentResult =
  | { readonly status: 'ok'; readonly data: unknown }
  /** The change waits for the person inside this lxapp; `page` opens where they review it. */
  | { readonly status: 'pending-review'; readonly summary: string; readonly page?: string; readonly data?: unknown }
  | { readonly status: 'error'; readonly code: string; readonly message: string };
