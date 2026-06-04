// Realtime client for Atomo's ephemeral tier (channels + presence + fan-out).
//
// Mirrors the server protocol exposed at `/realtime/ws` (see the Realtime
// Channels & Presence RFC). Framework-agnostic — React/Vue/etc. wrappers build
// on top. Payloads are opaque end to end; this client never inspects them.

/** Configuration for {@link RealtimeClient}. */
export interface RealtimeConfig {
  /** WebSocket URL, e.g. `ws://localhost:3000/realtime/ws`. */
  url: string;
  /** JWT sent as `?token=` on the handshake (the server's auth path). */
  token?: string;
  /** WebSocket implementation (inject for Node/tests); defaults to global. */
  WebSocketImpl?: typeof WebSocket;
  /** Reconnect with backoff after an unexpected close (default true). */
  reconnect?: boolean;
  /** Base reconnect delay in ms (default 500), capped at ~10s with backoff. */
  reconnectBaseMs?: number;
}

/** Handlers for a subscribed channel. All optional. */
export interface ChannelHandlers {
  /** A payload published to the channel by another participant. */
  onMessage?: (msg: { from?: string; payload: unknown }) => void;
  /** A participant joined the channel. */
  onJoined?: (who: string) => void;
  /** A participant left the channel. */
  onLeft?: (who: string) => void;
  /** Current membership — fired on snapshot and on every join/leave. */
  onPresence?: (members: string[]) => void;
}

type ServerFrame =
  | { type: 'message'; channel: string; from?: string; payload: unknown }
  | { type: 'joined'; channel: string; who: string }
  | { type: 'left'; channel: string; who: string }
  | { type: 'presence'; channel: string; members: string[] }
  | { type: 'error'; message: string };

const OPEN = 1;

/**
 * A connection to the realtime hub. Lazily connects on the first
 * {@link subscribe}/{@link publish}; auto-reconnects and re-subscribes.
 */
export class RealtimeClient {
  private readonly cfg: RealtimeConfig;
  private readonly WS: typeof WebSocket;
  private ws: WebSocket | null = null;
  private closedByUser = false;
  private reconnectAttempts = 0;
  private outbox: string[] = [];
  private readonly handlers = new Map<string, Set<ChannelHandlers>>();
  private readonly members = new Map<string, Set<string>>();
  /** Called for every server-sent error frame. */
  onError: (message: string) => void = () => {};

  constructor(config: RealtimeConfig) {
    this.cfg = config;
    const WS = config.WebSocketImpl ?? (globalThis as { WebSocket?: typeof WebSocket }).WebSocket;
    if (!WS) throw new Error('No WebSocket implementation available; pass config.WebSocketImpl');
    this.WS = WS;
  }

  /** Derive the realtime WS URL from a GraphQL endpoint (http→ws, /graphql→/realtime/ws). */
  static urlFromEndpoint(endpoint: string): string {
    return endpoint
      .replace(/^http/, 'ws')
      .replace(/\/graphql\/?$/, '')
      .replace(/\/$/, '') + '/realtime/ws';
  }

  /** Open the connection now. Safe to call repeatedly; a no-op if already live. */
  connect(): void {
    if (this.ws && this.ws.readyState <= OPEN) return;
    this.closedByUser = false;
    const url = this.cfg.token
      ? `${this.cfg.url}?token=${encodeURIComponent(this.cfg.token)}`
      : this.cfg.url;
    const ws = new this.WS(url);
    this.ws = ws;
    ws.onopen = () => {
      this.reconnectAttempts = 0;
      // Re-subscribe to every active channel, then flush queued frames.
      for (const channel of this.handlers.keys()) this.raw({ op: 'subscribe', channel });
      const pending = this.outbox;
      this.outbox = [];
      for (const frame of pending) ws.send(frame);
    };
    ws.onmessage = (ev: MessageEvent) => this.dispatch(ev.data);
    ws.onclose = () => {
      this.ws = null;
      if (this.closedByUser || this.cfg.reconnect === false) return;
      const base = this.cfg.reconnectBaseMs ?? 500;
      const delay = Math.min(base * 2 ** this.reconnectAttempts++, 10_000);
      setTimeout(() => this.connect(), delay);
    };
  }

  /**
   * Subscribe to a channel; returns an unsubscribe function. Multiple
   * subscriptions to the same channel each get their own handlers.
   */
  subscribe(channel: string, handlers: ChannelHandlers = {}): () => void {
    let set = this.handlers.get(channel);
    const firstForChannel = !set;
    if (!set) {
      set = new Set();
      this.handlers.set(channel, set);
    }
    set.add(handlers);
    this.connect();
    if (firstForChannel && this.isOpen()) this.raw({ op: 'subscribe', channel });

    return () => {
      const s = this.handlers.get(channel);
      if (!s) return;
      s.delete(handlers);
      if (s.size === 0) {
        this.handlers.delete(channel);
        this.members.delete(channel);
        if (this.isOpen()) this.raw({ op: 'unsubscribe', channel });
      }
    };
  }

  /** Publish an opaque payload to a channel (fans out to other subscribers). */
  publish(channel: string, payload: unknown): void {
    this.connect();
    this.raw({ op: 'publish', channel, payload });
  }

  /** Request a one-shot membership snapshot (also delivered via onPresence). */
  requestPresence(channel: string): void {
    this.connect();
    this.raw({ op: 'presence', channel });
  }

  /** Current known members of a channel (updated from server frames). */
  getPresence(channel: string): string[] {
    return [...(this.members.get(channel) ?? [])];
  }

  /** Close the connection and stop reconnecting. */
  close(): void {
    this.closedByUser = true;
    this.ws?.close();
    this.ws = null;
  }

  private isOpen(): boolean {
    return !!this.ws && this.ws.readyState === OPEN;
  }

  /** Send a client frame now if open, otherwise queue it for the next open. */
  private raw(frame: Record<string, unknown>): void {
    const json = JSON.stringify(frame);
    if (this.isOpen()) this.ws!.send(json);
    else this.outbox.push(json);
  }

  private membersOf(channel: string): Set<string> {
    let s = this.members.get(channel);
    if (!s) {
      s = new Set();
      this.members.set(channel, s);
    }
    return s;
  }

  private emitPresence(channel: string): void {
    const list = this.getPresence(channel);
    for (const h of this.handlers.get(channel) ?? []) h.onPresence?.(list);
  }

  private dispatch(data: unknown): void {
    let frame: ServerFrame;
    try {
      frame = JSON.parse(String(data));
    } catch {
      return;
    }
    if (frame.type === 'error') {
      this.onError(frame.message);
      return;
    }
    const subs = this.handlers.get(frame.channel);
    switch (frame.type) {
      case 'message':
        for (const h of subs ?? []) h.onMessage?.({ from: frame.from, payload: frame.payload });
        break;
      case 'joined':
        this.membersOf(frame.channel).add(frame.who);
        for (const h of subs ?? []) h.onJoined?.(frame.who);
        this.emitPresence(frame.channel);
        break;
      case 'left':
        this.membersOf(frame.channel).delete(frame.who);
        for (const h of subs ?? []) h.onLeft?.(frame.who);
        this.emitPresence(frame.channel);
        break;
      case 'presence':
        this.members.set(frame.channel, new Set(frame.members));
        this.emitPresence(frame.channel);
        break;
    }
  }
}
