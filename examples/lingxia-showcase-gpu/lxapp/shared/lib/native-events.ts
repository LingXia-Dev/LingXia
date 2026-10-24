// A native component reports through an event object whose payload rides on
// `detail`. Some components send the payload flat, so read it through here
// rather than repeating the fallback at every handler.
export type NativeEvent<T extends object> = { detail?: Partial<T> } & Partial<T>;

export function eventDetail<T extends object>(event?: NativeEvent<T>): Partial<T> {
  if (event && typeof event === "object" && event.detail && typeof event.detail === "object") {
    return event.detail;
  }
  return (event ?? {}) as Partial<T>;
}
