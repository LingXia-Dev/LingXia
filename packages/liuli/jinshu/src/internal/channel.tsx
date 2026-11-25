import { createContext, useContext, type ChannelEndpoint } from '@lingxia/view';
import type { ChannelClientMsg, ChannelServerMsg } from '../source/types';

export type JinshuChannelAction = (
  params: { readonly doc: string; readonly have?: number },
  channel: ChannelEndpoint<ChannelServerMsg, ChannelClientMsg>,
) => unknown;

const JinshuChannelContext = createContext<JinshuChannelAction | null>(null);

export interface JinshuProviderProps {
  readonly channel: JinshuChannelAction;
  readonly children: JSX.Element | null | false;
}

/** Shares a `syncDoc` channel action with descendant `<JinshuView>` / `<JinshuEditor>` instances. */
export function JinshuProvider({ channel, children }: JinshuProviderProps) {
  return <JinshuChannelContext.Provider value={channel}>{children}</JinshuChannelContext.Provider>;
}

export function useJinshuChannelContext(): JinshuChannelAction | null {
  return useContext(JinshuChannelContext);
}
