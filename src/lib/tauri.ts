// Typed invoke wrappers over the Rust command surface.
import { invoke } from '@tauri-apps/api/core';
import type { AppConfig, DeviceInfo, Entry4, KbOptions, Layout, LightingState } from './types';

export function probe() {
  return invoke<DeviceInfo | null>('probe');
}
export function disconnect() {
  return invoke<null>('disconnect');
}
export function lightGet(side: boolean) {
  return invoke<LightingState>('light_get', { side });
}
export function lightSet(side: boolean, state: LightingState) {
  return invoke<null>('light_set', { side, state });
}
export function ledsOn() {
  return invoke<KbOptions>('leds_on');
}
export function optionsGet() {
  return invoke<KbOptions>('options_get');
}
export function optionsSet(options: KbOptions) {
  return invoke<null>('options_set', { options });
}
export function profileGet() {
  return invoke<number>('profile_get');
}
export function profileSet(index: number) {
  return invoke<null>('profile_set', { index });
}
export function keymapRead(profile: number, fnLayer: boolean) {
  return invoke<number[]>('keymap_read', { profile, fnLayer });
}
export function keymapWrite(profile: number, fnLayer: boolean, slot: number, entry: Entry4) {
  return invoke<{ ok: boolean; restored?: boolean; before?: number[]; got?: number[] }>(
    'keymap_write',
    { profile, fnLayer, slot, entry },
  );
}
export function macroRead(slot: number) {
  return invoke<number[]>('macro_read', { slot });
}
export function macroWrite(slot: number, blob: number[]) {
  return invoke<null>('macro_write', { slot, blob });
}
export function displayParams() {
  return invoke<{ width: number | null; height: number | null; brightness?: number } | null>(
    'display_params',
  );
}
export function configGet() {
  return invoke<AppConfig>('config_get');
}
export function configSet(config: AppConfig) {
  return invoke<null>('config_set', { config });
}
export function logsGet() {
  return invoke<string[]>('logs_get');
}
export function layoutGet() {
  return invoke<Layout>('layout_get');
}
export function screenPush(rgbaB64: string, width: number, height: number, mode: string, order?: string, delay?: number, slot?: number) {
  return invoke<null>('screen_push', { rgbaB64, width, height, mode, order, delay, slot });
}
export function screenImport(path: string, mode: string, order?: string, slot?: number, slotCount?: number) {
  return invoke<{
    animated: boolean;
    count: number;
    previews: string[];
    delays: number[];
    width: number;
    height: number;
    slot: number;
  }>('screen_import', { path, mode, order, slot, slotCount });
}
export function screenPattern(pattern: string, slot?: number, width?: number, height?: number) {
  return invoke<{ preview: string; width: number; height: number }>('screen_pattern', { pattern, slot, width, height });
}
export function screenAnimPush(frames: { rgba_b64: string; delay: number }[], width: number, height: number, order?: string) {
  return invoke<null>('screen_anim_push', { frames, width, height, order });
}
export function screenClock(frames?: number, withSeconds?: boolean, color?: number[], background?: number[]) {
  return invoke<null>('screen_clock', { frames, withSeconds, color, background });
}
export function pickOpenImage() {
  return invoke<string | null>('pick_open_image');
}
export function pickOpenJson() {
  return invoke<string | null>('pick_open_json');
}
export function pickSaveFile(name: string) {
  return invoke<string | null>('pick_save_file', { name });
}
export function keymapBackup(path: string, profile: number) {
  return invoke<null>('keymap_backup', { path, profile });
}
export function keymapRestore(path: string) {
  return invoke<number>('keymap_restore', { path });
}

export const batteryProbe = () => invoke<number | null>('battery_probe');

export type HostInfo = {
  battery: number | null;
  charging: boolean;
  cpu: number;
  ram: number;
  uptime: number;
  hostname: string;
  firmware: string;
  profile: number;
  effect: number;
};

export const hostInfo = () => invoke<HostInfo>('host_info');
