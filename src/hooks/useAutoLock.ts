import { useEffect } from "react";

/** Default inactivity timeout before the vault auto-locks (THREAT F11). */
export const AUTO_LOCK_MS = 10 * 60 * 1000;

/**
 * Locks the vault after `delayMs` of no user activity. Any interaction resets
 * the timer. `onTimeout` must be stable (wrap in `useCallback`).
 */
export function useAutoLock(onTimeout: () => void, delayMs: number = AUTO_LOCK_MS) {
  useEffect(() => {
    let timer = window.setTimeout(onTimeout, delayMs);
    const reset = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(onTimeout, delayMs);
    };
    const events = ["mousemove", "mousedown", "keydown", "scroll", "touchstart"];
    events.forEach((e) => window.addEventListener(e, reset, { passive: true }));
    return () => {
      window.clearTimeout(timer);
      events.forEach((e) => window.removeEventListener(e, reset));
    };
  }, [onTimeout, delayMs]);
}
