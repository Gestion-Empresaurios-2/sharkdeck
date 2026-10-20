// Shared types mirroring the Rust structs (camelCase kept identical to serde output).

export type KbOptions = {
  win_lock: boolean;
  mac_mode: boolean;
  wasd_swap: boolean;
  main_led_off: boolean;
  side_led_off: boolean;
  keyboard_mode: boolean;
  keyboard_lock: boolean;
  fn_matrix: number;
  power_save: number;
};

export type LightingState = {
  effect: number;
  speed: number;
  brightness: number;
  direction: number;
  mode: number;
  r: number;
  g: number;
  b: number;
};

export type DeviceInfo = {
  path: string;
  deviceId?: string;
  handshakeId?: number;
  firmware?: string;
  profile?: number;
  options?: KbOptions;
  error?: string;
};

export type AppConfig = {
  screen_width: number;
  screen_height: number;
  order: 'Column' | 'Row';
  max_frames: number;
};

export type LayoutKey = {
  slot: number;
  code: string;
  text: string;
  x: number;
  y: number;
  w: number;
  h: number;
  entry: number[];
};

export type Layout = {
  canvas: { width: number; height: number };
  keys: LayoutKey[];
  unused: { matrixIndex: number; entry: number[] }[];
};

/** 4-byte firmware slot encoding. */
export type Entry4 = [number, number, number, number];

export const MODS = [
  { bit: 0x01, a: 'Ctrl', side: 'left' },
  { bit: 0x02, a: 'Shift', side: 'left' },
  { bit: 0x04, a: 'Alt', side: 'left' },
  { bit: 0x08, a: 'Super', side: 'left' },
  { bit: 0x10, a: 'Ctrl', side: 'right' },
  { bit: 0x20, a: 'Shift', side: 'right' },
  { bit: 0x40, a: 'Alt', side: 'right' },
  { bit: 0x80, a: 'Super', side: 'right' },
];

// HID usage ids we can bind (page 7).
export const HID_CODES: [number, string][] = [
  [0x04, 'a'], [0x05, 'b'], [0x06, 'c'], [0x07, 'd'], [0x08, 'e'], [0x09, 'f'],
  [0x0a, 'g'], [0x0b, 'h'], [0x0c, 'i'], [0x0d, 'j'], [0x0e, 'k'], [0x0f, 'l'],
  [0x10, 'm'], [0x11, 'n'], [0x12, 'o'], [0x13, 'p'], [0x14, 'q'], [0x15, 'r'],
  [0x16, 's'], [0x17, 't'], [0x18, 'u'], [0x19, 'v'], [0x1a, 'w'], [0x1b, 'x'],
  [0x1c, 'y'], [0x1d, 'z'],
  [0x1e, '1'], [0x1f, '2'], [0x20, '3'], [0x21, '4'], [0x22, '5'], [0x23, '6'],
  [0x24, '7'], [0x25, '8'], [0x26, '9'], [0x27, '0'],
  [0x28, 'Enter'], [0x29, 'Esc'], [0x2a, 'Backspace'], [0x2b, 'Tab'],
  [0x2c, 'Space'], [0x2d, '-'], [0x2e, '='], [0x2f, '['], [0x30, ']'],
  [0x31, '\\'], [0x32, '#'], [0x33, ';'], [0x34, "'"], [0x35, '`'],
  [0x36, ','], [0x37, '.'], [0x38, '/'], [0x39, 'Caps Lock'],
  [0x3a, 'F1'], [0x3b, 'F2'], [0x3c, 'F3'], [0x3d, 'F4'], [0x3e, 'F5'],
  [0x3f, 'F6'], [0x40, 'F7'], [0x41, 'F8'], [0x42, 'F9'], [0x43, 'F10'],
  [0x44, 'F11'], [0x45, 'F12'],
  [0x46, 'PrintScreen'], [0x47, 'Scroll Lock'], [0x48, 'Pause'],
  [0x49, 'Insert'], [0x4a, 'Home'], [0x4b, 'PageUp'], [0x4c, 'Delete'],
  [0x4d, 'End'], [0x4e, 'PageDown'], [0x4f, '→'], [0x50, '←'], [0x51, '↓'],
  [0x52, '↑'], [0x57, 'Keypad Enter'], [0x53, 'NumLock'],
  [0x54, 'Keypad /'], [0x55, 'Keypad *'], [0x56, 'Keypad -'],
  [0x58, 'Keypad +'], [0x5c, 'Keypad 2'], [0x59, 'Keypad 1'],
  [0x5a, 'Keypad 3'], [0x5b, 'Keypad 4'], [0x5d, 'Keypad 6'],
  [0x5e, 'Keypad 7'], [0x5f, 'Keypad 8'], [0x60, 'Keypad 9'],
  [0x61, 'Keypad 0'], [0x62, 'Keypad .'],
];

export const CONSUMER_CODES: [string, string][] = [
  ['00e9', 'Volume Up'],
  ['00ea', 'Volume Down'],
  ['00e2', 'Mute'],
  ['00cd', 'Play / Pause'],
  ['00b5', 'Next Track'],
  ['00b6', 'Previous Track'],
  ['00b7', 'Stop'],
  ['006f', 'Brightness Up'],
  ['0070', 'Brightness Down'],
  ['0221', 'Search'],
  ['0192', 'Calculator'],
  ['0196', 'Browser'],
];

export const FIRMWARE_ACTIONS: [string, string][] = [
  ['06', 'Office / Gaming mode switch'],
  ['07', 'Keyboard lock'],
  ['08', 'Check battery'],
  ['09', 'LED on / off'],
  ['10', 'WASD change'],
  ['11', 'Fn key matrix change'],
  ['12', 'Power save toggle'],
  ['13', 'Fn lock'],
  ['14', 'Wheel swap (↕ volume ↔ brightness)'],
  ['15', 'Caps swap'],
  ['16', 'Sleep toggle'],
  ['17', 'Caps / LED swap'],
  ['18', 'Power down'],
  ['19', 'Fn key swap'],
  ['20', 'Alt+Tab'],
  ['21', 'Language switch'],
];

export function hexCode(n: number, up: boolean): string {
  const h = n.toString(16).padStart(4, '0');
  return up ? h.toUpperCase() : h;
}

export function usageLabel(u: number): string {
  const hit = HID_CODES.find(([c]) => c === u);
  return hit ? hit[1] : `Usage 0x${hexCode(u, true)}`;
}

export function modsLabel(m: number): string {
  if (!m) return '';
  const parts = MODS.filter((x) => m & x.bit).map((x) => x.a.replace(' ', ''));
  return parts.join('+');
}
