import { describe, it, expect, vi, beforeEach } from 'vitest'
import { OfflineQueue } from './offline-queue'

// Minimal browser globals for the queue (no real browser needed).
const storage: Record<string, string> = {}
vi.stubGlobal('localStorage', {
  getItem: (k: string) => storage[k] ?? null,
  setItem: (k: string, v: string) => { storage[k] = v },
})
vi.stubGlobal('navigator', { onLine: true })
vi.stubGlobal('window', { addEventListener: vi.fn() })
vi.stubGlobal('crypto', { randomUUID: () => 'test-uuid-1' })

describe('OfflineQueue', () => {
  let queue: OfflineQueue
  let fetches: { url: string; body: any }[]

  beforeEach(() => {
    delete storage['atomo_offline_queue']
    fetches = []
    vi.stubGlobal('fetch', vi.fn(async (url: string, opts: any) => {
      fetches.push({ url, body: JSON.parse(opts.body) })
      return { ok: true }
    }))
    vi.stubGlobal('navigator', { onLine: true })
    queue = new OfflineQueue('http://localhost:3000')
    queue.setAuthToken('tok-1')
  })

  it('enqueues and syncs a create mutation', async () => {
    queue.enqueue('Contact', 'create', { firstName: 'Ada' })
    // enqueue auto-triggers sync(); give it a tick to complete
    await new Promise(r => setTimeout(r, 50))
    expect(fetches).toHaveLength(1)
    expect(fetches[0].body.variables.model).toBe('Contact')
    expect(fetches[0].body.variables.data.firstName).toBe('Ada')
    expect(queue.getStatus().pendingCount).toBe(0)
  })

  it('persists to localStorage and reloads', () => {
    vi.stubGlobal('navigator', { onLine: false })
    queue = new OfflineQueue('http://localhost:3000')
    queue.enqueue('Deal', 'update', { id: 'd1', title: 'X' })
    expect(queue.getStatus().pendingCount).toBe(1)
    // Simulate reload
    const q2 = new OfflineQueue('http://localhost:3000')
    expect(q2.getStatus().pendingCount).toBe(1)
  })

  it('retries on failure and drops after 5 retries', async () => {
    const mockFetch = vi.fn(async () => ({ ok: false }))
    vi.stubGlobal('fetch', mockFetch)
    vi.stubGlobal('navigator', { onLine: true })
    queue = new OfflineQueue('http://localhost:3000')
    // enqueue auto-triggers first sync (which fails). Then loop the remaining syncs.
    queue.enqueue('Contact', 'delete', { id: 'c1' })
    // Wait for auto-sync to complete
    await new Promise(r => setTimeout(r, 50))
    // Each sync increments retries; after 5 total fails the item is dropped.
    for (let i = 0; i < 5; i++) {
      await queue.sync()
    }
    expect(queue.getStatus().pendingCount).toBe(0)
  })

  it('does not fetch when offline', async () => {
    const mockFetch = vi.fn(async () => ({ ok: true }))
    vi.stubGlobal('fetch', mockFetch)
    vi.stubGlobal('navigator', { onLine: false })
    queue = new OfflineQueue('http://localhost:3000')
    queue.enqueue('Contact', 'create', { firstName: 'B' })
    await new Promise(r => setTimeout(r, 50))
    await queue.sync()
    expect(mockFetch).not.toHaveBeenCalled()
    expect(queue.getStatus().pendingCount).toBe(1)
  })
})
