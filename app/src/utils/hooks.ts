import { useEffect, useState } from "react";

/** Re-renders the component every `ms` milliseconds; returns a counter. */
export function useTick(ms: number): number {
  const [tick, setTick] = useState(0);
  useEffect(() => {
    const id = window.setInterval(() => setTick((n) => n + 1), ms);
    return () => window.clearInterval(id);
  }, [ms]);
  return tick;
}
