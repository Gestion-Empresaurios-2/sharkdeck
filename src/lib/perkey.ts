// Typed invoke wrappers for the per-key RGB (SET_USERPIC / 0x0C) commands.
import { invoke } from '@tauri-apps/api/core';

/** One key of the K86 layout, projected onto the colour grid. */
export type PerKeyCell = {
  slot: number;
  code: string;
  text: string;
  col: number;
  row: number;
  cell: number;
  x: number;
  y: number;
  w: number;
  h: number;
};

/** Flash-upload limiter state (2 uploads per 10 s). */
export type PerKeyUploadState = {
  allowed: boolean;
  remaining: number;
  max: number;
  windowS: number;
  nextAllowedInS: number;
};

export type PerKeyGrid = {
  width: number;
  height: number;
  canvas: { width: number; height: number };
  keyCount: number;
  cells: PerKeyCell[];
  unusedSlots: number[];
  uploads: PerKeyUploadState;
};

export type PerKeyPushResult = {
  sent: boolean;
  dry_run: boolean;
  bytes: number;
  chunks: number;
  keys: number;
  remaining: number;
  next_allowed_in_s: number;
};

/** Grid + cell rectangles of the K86 layout, plus the upload limiter state. */
export function perkeyGrid() {
  return invoke<PerKeyGrid>('perkey_grid');
}

/**
 * Encode (and optionally upload) a per-key colour map.
 * `cells` holds one `[r, g, b]` triple per grid cell in row-major order.
 * With `dryRun` nothing is written to the keyboard.
 */
export function perkeyPush(cells: [number, number, number][], dryRun: boolean) {
  return invoke<PerKeyPushResult>('perkey_push', { cells, dryRun });
}
