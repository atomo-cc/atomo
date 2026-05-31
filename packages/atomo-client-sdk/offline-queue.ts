/**
 * Local-first offline queue with sync-on-reconnect
 * Queues mutations when offline and replays them when connection is restored.
 */

export interface QueuedMutation {
  id: string;
  model: string;
  operation: 'create' | 'update' | 'delete';
  data: Record<string, any>;
  timestamp: number;
  retries: number;
}

export interface SyncStatus {
  online: boolean;
  pendingCount: number;
  lastSyncAt: number | null;
  syncing: boolean;
}

export type SyncListener = (status: SyncStatus) => void;

export class OfflineQueue {
  private queue: QueuedMutation[] = [];
  private storageKey = 'atomo_offline_queue';
  private listeners: SyncListener[] = [];
  private syncing = false;
  private endpoint: string;
  private authToken: string | null = null;

  constructor(endpoint: string) {
    this.endpoint = endpoint;
    this.loadFromStorage();
    this.setupConnectivityListeners();
  }

  setAuthToken(token: string | null) {
    this.authToken = token;
  }

  /** Queue a mutation for later sync */
  enqueue(model: string, operation: 'create' | 'update' | 'delete', data: Record<string, any>): string {
    const id = crypto.randomUUID?.() || `${Date.now()}-${Math.random().toString(36).slice(2)}`;
    this.queue.push({ id, model, operation, data, timestamp: Date.now(), retries: 0 });
    this.saveToStorage();
    this.notifyListeners();
    if (navigator.onLine) this.sync();
    return id;
  }

  /** Attempt to sync all queued mutations */
  async sync(): Promise<void> {
    if (this.syncing || this.queue.length === 0 || !navigator.onLine) return;
    this.syncing = true;
    this.notifyListeners();

    const toSync = [...this.queue];
    const failed: QueuedMutation[] = [];

    for (const mutation of toSync) {
      try {
        await this.executeMutation(mutation);
        this.queue = this.queue.filter(m => m.id !== mutation.id);
      } catch {
        mutation.retries++;
        if (mutation.retries < 5) failed.push(mutation);
      }
    }

    this.queue = failed;
    this.syncing = false;
    this.saveToStorage();
    this.notifyListeners();
  }

  private async executeMutation(mutation: QueuedMutation): Promise<void> {
    const { model, operation, data } = mutation;
    const queries: Record<string, string> = {
      create: `mutation($model: String!, $data: JSON!) { create(model: $model, data: $data) }`,
      update: `mutation($model: String!, $where: JSON!, $data: JSON!) { update(model: $model, where: $where, data: $data) }`,
      delete: `mutation($model: String!, $where: JSON!) { delete(model: $model, where: $where) }`,
    };
    const variables: any = { model };
    if (operation === 'create') { variables.data = data; }
    else if (operation === 'update') { variables.where = { id: { equals: data.id } }; variables.data = data; }
    else { variables.where = { id: { equals: data.id } }; }

    const headers: Record<string, string> = { 'Content-Type': 'application/json' };
    if (this.authToken) headers['Authorization'] = `Bearer ${this.authToken}`;

    const res = await fetch(`${this.endpoint}/graphql`, {
      method: 'POST',
      headers,
      body: JSON.stringify({ query: queries[operation], variables }),
    });
    if (!res.ok) throw new Error(`Sync failed: ${res.status}`);
  }

  /** Subscribe to sync status changes */
  onStatusChange(listener: SyncListener): () => void {
    this.listeners.push(listener);
    return () => { this.listeners = this.listeners.filter(l => l !== listener); };
  }

  getStatus(): SyncStatus {
    return { online: navigator.onLine, pendingCount: this.queue.length, lastSyncAt: null, syncing: this.syncing };
  }

  private notifyListeners() {
    const status = this.getStatus();
    this.listeners.forEach(l => l(status));
  }

  private setupConnectivityListeners() {
    if (typeof window !== 'undefined') {
      window.addEventListener('online', () => this.sync());
    }
  }

  private loadFromStorage() {
    try {
      const stored = localStorage.getItem(this.storageKey);
      if (stored) this.queue = JSON.parse(stored);
    } catch {}
  }

  private saveToStorage() {
    try { localStorage.setItem(this.storageKey, JSON.stringify(this.queue)); } catch {}
  }
}
