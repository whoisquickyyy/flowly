// Транспорт Jam: Together (mashiro-релей + LAN) из yandex-music-client.
// Руст-модуль владеет сокетами/шифрованием; фронтенд шлёт payloads и слушает события.

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { JamSyncPayload } from './jamSync';

export type JamTransportKind = 'mashiro' | 'lan';

export interface TogetherPeer {
  id: number;
  nick: string;
}

export interface TogetherStatus {
  mode: 'off' | 'host' | 'guest';
  port: number;
  nick: string;
  address: string | null;
  selfId: number;
  peers: TogetherPeer[];
}

export interface RelayPeer {
  id: number;
  nick: string;
}

export interface RelayStatus {
  connected: boolean;
  address: string;
  room: string;
  invite: string;
  nick: string;
  selfId: number;
  host: number;
  peers: RelayPeer[];
  ping: number;
}

export interface TogetherMessage {
  from: number;
  nick: string;
  payload: JamSyncPayload | null;
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw new Error(typeof e === 'string' ? e : 'Ошибка вызова ядра');
  }
}

/** LAN-режим: прямой TCP между участниками (работает и через Radmin/VPN-локалку) */
export const lanApi = {
  host: (nick: string, port?: number | null) =>
    call<TogetherStatus>('together_host', { nick, port: port ?? null }),
  join: (address: string, nick: string) =>
    call<TogetherStatus>('together_join', { address, nick }),
  leave: () => call<TogetherStatus>('together_leave'),
  send: (payload: unknown) => call<void>('together_send', { payload }),
  status: () => call<TogetherStatus>('together_status'),
  peerAddress: (id: number) => call<string | null>('together_peer_address', { id }),
  logPath: () => call<string>('together_log_path'),
};

/** Релей mashiro-together: анонимный вход по сид-фразе, payload шифруется на клиенте */
export const relayApi = {
  create: (address: string, nick: string, invite?: string) =>
    call<RelayStatus>('together_relay_create', { address, nick, invite: invite ?? null }),
  join: (address: string, nick: string, invite: string) =>
    call<RelayStatus>('together_relay_join', { address, nick, invite }),
  leave: () => invoke<void>('together_relay_leave'),
  send: (payload: unknown) => invoke<void>('together_relay_send', { payload }),
  handoff: (to: number) => invoke<void>('together_relay_handoff', { to }),
  serverStart: (port: number) =>
    invoke<{ address: string; port: number }>('relay_server_start', { port }),
  serverStop: () => invoke<void>('relay_server_stop'),
  serverRunning: () => invoke<boolean>('relay_server_running'),
  serverLocalIp: () => invoke<string | null>('relay_server_local_ip'),
  status: () => invoke<RelayStatus>('together_relay_status'),
  seedExists: () => invoke<boolean>('together_relay_seed_exists'),
  seedShow: () => invoke<string | null>('together_relay_seed_show'),
  seedSet: (phrase: string) => invoke<string>('together_relay_seed_set', { phrase }),
  seedNew: (words: 12 | 24 = 12) => invoke<string>('together_relay_seed_new', { words }),
  seedForget: () => invoke<void>('together_relay_seed_forget'),
  identity: () => invoke<string | null>('together_relay_identity'),
  invite: (short = false) => invoke<string>('together_relay_invite', { short }),
};

export function emptyRelayStatus(): RelayStatus {
  return {
    connected: false,
    address: '',
    room: '',
    invite: '',
    nick: '',
    selfId: 0,
    host: 0,
    peers: [],
    ping: 0,
  };
}

export function onTogetherMessage(
  handler: (message: TogetherMessage) => void,
): Promise<UnlistenFn> {
  return listen<TogetherMessage>('together://message', (event) => handler(event.payload));
}

export function onRelayMessage(handler: (message: TogetherMessage) => void): Promise<UnlistenFn> {
  return listen<TogetherMessage>('together://relay-message', (event) => handler(event.payload));
}

export function onTogetherStatus(handler: (status: TogetherStatus) => void): Promise<UnlistenFn> {
  return listen<TogetherStatus>('together://status', (event) => handler(event.payload));
}

export function onRelayStatus(handler: (status: RelayStatus) => void): Promise<UnlistenFn> {
  return listen<RelayStatus>('together://relay-status', (event) => handler(event.payload));
}

export function onRelayJoined(handler: (peer: { id: number; nick: string }) => void): Promise<UnlistenFn> {
  return listen<{ id: number; nick: string }>('together://relay-joined', (event) => handler(event.payload));
}

export function onRelayClosed(handler: (closed: { reason: string }) => void): Promise<UnlistenFn> {
  return listen<{ reason: string }>('together://relay-closed', (event) => handler(event.payload));
}

export function onTogetherClosed(handler: (closed: { reason: string }) => void): Promise<UnlistenFn> {
  return listen<{ reason: string }>('together://closed', (event) => handler(event.payload));
}

export function onTogetherLog(handler: (log: { target: string; message: string }) => void): Promise<UnlistenFn> {
  return listen<{ target: string; message: string }>('together://log', (event) => handler(event.payload));
}
