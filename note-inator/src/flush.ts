// The open editor registers a function that saves pending edits immediately.
let fn: (() => Promise<void>) | null = null;
export const registerFlush = (f: () => Promise<void>) => { fn = f; };
export const unregisterFlush = () => { fn = null; };
export const flushNow = async () => { if (fn) await fn(); };
