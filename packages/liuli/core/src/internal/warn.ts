// The page runtime has a console; Liuli's types leave out the DOM's.
declare const console: { warn(message: string): void };

const said = new Set<string>();

/** Tells the author about a mistake the types cannot see, once per message. */
export function warnOnce(message: string) {
  if (said.has(message)) return;
  said.add(message);
  console.warn(`Liuli: ${message}`);
}
