// Live reverse-engineering tracker.
//
// Every entry is a question we are trying to answer about the Attack Shark K86
// firmware, with the state we have actually reached. `done` means the answer
// is measured and written down (see the `re/` notes in the repository);
// `blocked` means the path is closed off by the hardware/vendor; `open` means
// the next experiment is known but not yet run.

export type ReStatus = 'done' | 'open' | 'blocked';

export interface ReItem {
  id: string;
  track: string;
  title: string;
  status: ReStatus;
  note: string;
  source?: string;
}

export const RE_TRACKS = [
  'Hardware truth',
  'Protocol',
  'Firmware access',
  'Native OS',
  'Desktop app',
] as const;

export const RE_ITEMS: ReItem[] = [
  // ---------------------------------------------------------------- hardware
  {
    id: 'hw-ident',
    track: 'Hardware truth',
    title: 'Board identity, firmware and panel geometry',
    status: 'done',
    note: 'dev 1168, fw 1.19, 240×135 RGB565 column-major, 5 picture slots, 3 profiles.',
    source: 're/OPCODE-FINDINGS.md',
  },
  {
    id: 'hw-topology',
    track: 'Hardware truth',
    title: 'USB / HID topology and report descriptors',
    status: 'done',
    note: '3 interfaces: boot keyboard, consumer/mouse (knob), and the 64-byte vendor control channel.',
    source: 're/PROTOCOL-AUDIT.md',
  },
  {
    id: 'hw-knob-node',
    track: 'Hardware truth',
    title: 'Knob exposed as a mouse-style HID device',
    status: 'done',
    note: 'Kernel node /dev/input/event12 with REL_X/Y/Z/RY/DIAL/WHEEL + buttons.',
    source: 're/OPCODE-FINDINGS.md',
  },
  {
    id: 'hw-battery',
    track: 'Hardware truth',
    title: 'Battery telemetry source',
    status: 'blocked',
    note: 'No battery read opcode exists; the K86 only reports while USB-powered. Host battery is used instead.',
  },

  // ---------------------------------------------------------------- protocol
  {
    id: 'pr-inventory',
    track: 'Protocol',
    title: 'Full opcode inventory across public sources',
    status: 'done',
    note: '92 distinct opcodes mapped, 32 confirmed exercised on this board.',
    source: 're/PROTOCOL-AUDIT.md',
  },
  {
    id: 'pr-baseline',
    track: 'Protocol',
    title: 'Baseline-diff probe for the whole GET family',
    status: 'done',
    note: '0x8F poison + reply diff separates implemented opcodes from replayed (stale) ones.',
    source: 're/OPCODE-FINDINGS.md',
  },
  {
    id: 'pr-checksum',
    track: 'Protocol',
    title: 'Reply checksum convention confirmed',
    status: 'done',
    note: 'reply[7] = 0xFF − Σ(reply[0..7]); 0x92 uses a different payload layout.',
  },
  {
    id: 'pr-unknown',
    track: 'Protocol',
    title: 'Decode the undocumented opcodes',
    status: 'open',
    note: '0x92 (telemetry block), 0x83/0x84 (counters), 0x8D/0x82 (empty), 0x91 (debounce).',
    source: 're/OPCODE-FINDINGS.md',
  },
  {
    id: 'pr-picture-index',
    track: 'Protocol',
    title: 'Find the "show picture N" opcode',
    status: 'open',
    note: '0x20 / 0xA0 are the only candidates; needs the payload format before it is safe to send.',
  },
  {
    id: 'pr-display-params',
    track: 'Protocol',
    title: 'Read display parameters (0x0E)',
    status: 'blocked',
    note: 'The Swift app uses an IOKit global report index (13), which has no hidraw equivalent.',
  },
  {
    id: 'pr-native-events',
    track: 'Protocol',
    title: 'Native event channel (knob turns, menu, battery)',
    status: 'open',
    note: 'Raw vendor INPUT report ID 5 carries the axis data; decoder in place, waiting for a live capture.',
  },

  // ---------------------------------------------------------- firmware access
  {
    id: 'fw-r1',
    track: 'Firmware access',
    title: 'R1 — read the flash over HID',
    status: 'done',
    note: 'Proven impossible: the dispatcher has no read command. Route closed.',
    source: 're/SHARKFIN-FLASH-AUDIT.md',
  },
  {
    id: 'fw-r2',
    track: 'Firmware access',
    title: 'R2 — bootloader DFU read',
    status: 'blocked',
    note: 'The bootloader can only write and verify (0xBA init/data/verify); there is no read-back.',
    source: 're/SHARKFIN-FLASH-AUDIT.md',
  },
  {
    id: 'fw-r3',
    track: 'Firmware access',
    title: 'R3 — SWD/JTAG clip on the MCU',
    status: 'open',
    note: 'Needs physical access to the board and a clip; the only route to a real image of our unit.',
  },
  {
    id: 'fw-r4',
    track: 'Firmware access',
    title: 'R4 — stock image for our dev id (1168)',
    status: 'blocked',
    note: 'Vendor channel answers HTTP 500 "Record not found" for our board; the K86 v2 (dev 2730) image was obtained.',
    source: 're/K86-FIRMWARE.md',
  },
  {
    id: 'fw-container',
    track: 'Firmware access',
    title: 'Decode the G1H packed container',
    status: 'open',
    note: 'Chunk table tagged 0x010003e5; no deflate/lzma carving; ARM vector not visible yet.',
    source: 're/K86-FIRMWARE.md',
  },
  {
    id: 'fw-resources',
    track: 'Firmware access',
    title: 'Locate the built-in screen resources',
    status: 'open',
    note: 'Look for 240×135 RGB565 blobs (64,800 B) inside the OLED-controller firmware.',
  },

  // ---------------------------------------------------------------- native OS
  {
    id: 'no-settings',
    track: 'Native OS',
    title: 'Backup and restore every setting',
    status: 'done',
    note: '3 profiles × base + Fn layers, 50 macros, options, lighting — verified on the board.',
  },
  {
    id: 'no-denylist',
    track: 'Native OS',
    title: 'Hard denylist for destructive opcodes',
    status: 'done',
    note: 'Erase / reset / boot opcodes are refused inside the transport layer itself.',
  },
  {
    id: 'no-knob-listener',
    track: 'Native OS',
    title: 'Native knob listener (evdev + raw HID fallback)',
    status: 'done',
    note:
      'Solved: the knob arrives as consumer-control keys (115 = turn right, 114 = turn left, 113 = press) on the Consumer Control node. A udev rule now gives plugdev read access to the keyboard input nodes, so no "input" group is needed.',
  },
  {
    id: 'no-menu',
    track: 'Native OS',
    title: 'Reach the keyboard’s own menu programmatically',
    status: 'open',
    note: 'Today the menu is opened manually with Fn + knob press; no opcode reproduces it.',
  },
  {
    id: 'no-replace-screens',
    track: 'Native OS',
    title: 'Replace the built-in screens with a native FRAME OS',
    status: 'blocked',
    note: 'Requires the container format and a writable OLED-controller image; highest-risk work.',
  },

  // -------------------------------------------------------------- desktop app
  {
    id: 'app-notion',
    track: 'Desktop app',
    title: 'Notion design system for the desktop shell',
    status: 'done',
    note: 'Top-nav shell, hairline cards, blue accent, 64px bar, light + dark.',
  },
  {
    id: 'app-frame-os',
    track: 'Desktop app',
    title: 'FRAME OS renderer (9 screens) + Screen OS studio',
    status: 'done',
    note: 'Rendered natively at 240×135 with wallpapers, accents, lock layouts and complications.',
  },
  {
    id: 'app-live',
    track: 'Desktop app',
    title: 'Live metrics and correct clock pacing',
    status: 'done',
    note: 'Real host telemetry, 15 fps preview, 15-frame absolute-timestamp window, 200 µs chunk pacing.',
  },
  {
    id: 'app-opcode-lab',
    track: 'Desktop app',
    title: 'Opcode lab in the UI',
    status: 'done',
    note: 'Baseline-diff probe and full reply dump, with the denylist surfaced.',
  },
  {
    id: 'app-updater',
    track: 'Desktop app',
    title: 'Signed auto-update pipeline',
    status: 'done',
    note: 'CI builds AppImage + deb, signs them and publishes latest.json for the in-app updater.',
  },
];

export interface ReTrackProgress {
  track: string;
  total: number;
  done: number;
  open: number;
  blocked: number;
  pct: number;
}

export interface ReProgress {
  total: number;
  done: number;
  open: number;
  blocked: number;
  pct: number;
  tracks: ReTrackProgress[];
}

export function reProgress(items: ReItem[] = RE_ITEMS): ReProgress {
  const tracks = RE_TRACKS.map((track) => {
    const list = items.filter((i) => i.track === track);
    const done = list.filter((i) => i.status === 'done').length;
    const open = list.filter((i) => i.status === 'open').length;
    const blocked = list.filter((i) => i.status === 'blocked').length;
    const total = list.length;
    return { track, total, done, open, blocked, pct: total ? Math.round((done / total) * 100) : 0 };
  });
  const total = items.length;
  const done = items.filter((i) => i.status === 'done').length;
  const open = items.filter((i) => i.status === 'open').length;
  const blocked = items.filter((i) => i.status === 'blocked').length;
  return {
    total,
    done,
    open,
    blocked,
    pct: total ? Math.round((done / total) * 100) : 0,
    tracks,
  };
}

export const RE_STATUS_LABEL: Record<ReStatus, string> = {
  done: 'Answered',
  open: 'Next up',
  blocked: 'Closed off',
};
