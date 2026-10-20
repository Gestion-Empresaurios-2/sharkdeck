// Live desk + device telemetry, polled from the Rust side.
// Plain module (no runes): the panel renderers read `live.info` on every
// frame, so a polled snapshot is enough.

import { hostInfo, type HostInfo } from './tauri';

export const live: { info: HostInfo | null } = { info: null };

let timer: ReturnType<typeof setInterval> | null = null;

/** Poll host + keyboard telemetry so on-panel widgets have real numbers. */
export function startLive(intervalMs = 2000): void {
  if (timer !== null) return;
  const tick = async () => {
    try {
      live.info = await hostInfo();
    } catch {
      /* keep the last good snapshot */
    }
  };
  void tick();
  timer = setInterval(tick, intervalMs);
}

export function stopLive(): void {
  if (timer !== null) {
    clearInterval(timer);
    timer = null;
  }
}
