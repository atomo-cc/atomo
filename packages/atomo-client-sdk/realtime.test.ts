import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { RealtimeClient } from './realtime';

// Minimal WebSocket double: records sent frames, lets tests drive open/message/close.
class MockWebSocket {
  static instances: MockWebSocket[] = [];
  static last(): MockWebSocket {
    return MockWebSocket.instances[MockWebSocket.instances.length - 1];
  }

  url: string;
  readyState = 0; // CONNECTING
  sent: string[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((ev: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;

  constructor(url: string) {
    this.url = url;
    MockWebSocket.instances.push(this);
  }
  send(data: string) {
    this.sent.push(data);
  }
  close() {
    this.readyState = 3; // CLOSED
    this.onclose?.();
  }

  // --- test drivers ---
  open() {
    this.readyState = 1; // OPEN
    this.onopen?.();
  }
  emit(frame: unknown) {
    this.onmessage?.({ data: JSON.stringify(frame) });
  }
  /** Parsed frames the client sent. */
  frames() {
    return this.sent.map((s) => JSON.parse(s));
  }
}

function newClient(opts: Partial<ConstructorParameters<typeof RealtimeClient>[0]> = {}) {
  return new RealtimeClient({
    url: 'ws://test/realtime/ws',
    WebSocketImpl: MockWebSocket as unknown as typeof WebSocket,
    reconnect: false,
    ...opts,
  });
}

beforeEach(() => {
  MockWebSocket.instances = [];
});

describe('RealtimeClient', () => {
  it('derives the realtime URL from a GraphQL endpoint', () => {
    expect(RealtimeClient.urlFromEndpoint('http://localhost:3000/graphql')).toBe(
      'ws://localhost:3000/realtime/ws',
    );
    expect(RealtimeClient.urlFromEndpoint('https://api.example.com/graphql')).toBe(
      'wss://api.example.com/realtime/ws',
    );
  });

  it('appends the token to the handshake URL', () => {
    const c = newClient({ token: 'jwt-123' });
    c.connect();
    expect(MockWebSocket.last().url).toBe('ws://test/realtime/ws?token=jwt-123');
  });

  it('queues a subscribe frame before open and sends it on open', () => {
    const c = newClient();
    c.subscribe('deal:1');
    const ws = MockWebSocket.last();
    expect(ws.frames()).toEqual([]); // nothing sent while CONNECTING
    ws.open();
    expect(ws.frames()).toEqual([{ op: 'subscribe', channel: 'deal:1' }]);
  });

  it('routes message frames only to the matching channel handler', () => {
    const c = newClient();
    const onMessage = vi.fn();
    c.subscribe('deal:1', { onMessage });
    c.subscribe('deal:2', { onMessage: vi.fn() });
    MockWebSocket.last().open();

    MockWebSocket.last().emit({ type: 'message', channel: 'deal:1', from: 'alice', payload: { x: 1 } });
    expect(onMessage).toHaveBeenCalledTimes(1);
    expect(onMessage).toHaveBeenCalledWith({ from: 'alice', payload: { x: 1 } });
  });

  it('tracks presence across snapshot, joined, and left frames', () => {
    const c = newClient();
    const onPresence = vi.fn();
    c.subscribe('room', { onPresence });
    const ws = MockWebSocket.last();
    ws.open();

    ws.emit({ type: 'presence', channel: 'room', members: ['alice', 'bob'] });
    expect(c.getPresence('room')).toEqual(['alice', 'bob']);

    ws.emit({ type: 'joined', channel: 'room', who: 'carol' });
    expect(c.getPresence('room')).toEqual(['alice', 'bob', 'carol']);

    ws.emit({ type: 'left', channel: 'room', who: 'alice' });
    expect(c.getPresence('room')).toEqual(['bob', 'carol']);

    // onPresence fired for each of the three membership changes.
    expect(onPresence).toHaveBeenCalledTimes(3);
    expect(onPresence).toHaveBeenLastCalledWith(['bob', 'carol']);
  });

  it('publish sends an opaque payload frame', () => {
    const c = newClient();
    c.connect();
    MockWebSocket.last().open();
    c.publish('room', { typing: true });
    expect(MockWebSocket.last().frames()).toContainEqual({
      op: 'publish',
      channel: 'room',
      payload: { typing: true },
    });
  });

  it('unsubscribe sends unsubscribe only when the last handler for a channel leaves', () => {
    const c = newClient();
    const off1 = c.subscribe('room');
    const off2 = c.subscribe('room');
    const ws = MockWebSocket.last();
    ws.open();
    ws.sent = [];

    off1(); // one handler remains → no unsubscribe
    expect(ws.frames()).toEqual([]);
    off2(); // last handler → unsubscribe
    expect(ws.frames()).toEqual([{ op: 'unsubscribe', channel: 'room' }]);
  });

  it('routes error frames to onError', () => {
    const c = newClient();
    const onError = vi.fn();
    c.onError = onError;
    c.connect();
    MockWebSocket.last().open();
    MockWebSocket.last().emit({ type: 'error', message: 'bad frame' });
    expect(onError).toHaveBeenCalledWith('bad frame');
  });

  describe('reconnect', () => {
    beforeEach(() => vi.useFakeTimers());
    afterEach(() => vi.useRealTimers());

    it('reconnects after an unexpected close and re-subscribes active channels', () => {
      const c = newClient({ reconnect: true, reconnectBaseMs: 100 });
      c.subscribe('deal:1');
      const first = MockWebSocket.last();
      first.open();
      expect(first.frames()).toEqual([{ op: 'subscribe', channel: 'deal:1' }]);

      first.close(); // unexpected drop
      vi.advanceTimersByTime(100); // backoff elapses → new connection
      const second = MockWebSocket.last();
      expect(second).not.toBe(first);
      second.open();
      // The fresh socket re-subscribes without the caller doing anything.
      expect(second.frames()).toEqual([{ op: 'subscribe', channel: 'deal:1' }]);
    });

    it('does not reconnect after close()', () => {
      const c = newClient({ reconnect: true, reconnectBaseMs: 100 });
      c.subscribe('deal:1');
      MockWebSocket.last().open();
      const count = MockWebSocket.instances.length;

      c.close();
      vi.advanceTimersByTime(1000);
      expect(MockWebSocket.instances.length).toBe(count); // no new socket
    });
  });
});
