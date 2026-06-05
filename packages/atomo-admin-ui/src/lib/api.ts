/**
 * Atomo API Client
 *
 * Unified API client for communicating with Atomo Core.
 */

import axios, { AxiosInstance } from 'axios'
import { SchemaMetadata, EntityData, QueryOptions } from './types'
import { loadSchemaMetadata } from './schema-parser'
import { cloneDemoEntities } from './demo-data'

/** The signed-in user, as returned by GET /auth/me. */
export interface AuthUser {
  id: string
  email: string
  role: string
  first_name?: string
  last_name?: string
}

class AtomoApiClient {
  private client: AxiosInstance
  private baseUrl: string
  private demoStore: Record<string, EntityData[]> = {}
  /** True once any request has fallen back to in-memory demo data (no live backend). */
  public usedDemoData = false

  constructor(baseUrl: string = '') {
    // Simplified URL detection - more reliable than complex logic
    this.baseUrl = baseUrl || this.getApiBaseUrl()
    this.client = axios.create({
      baseURL: this.baseUrl,
      headers: {
        'Content-Type': 'application/json',
      },
      timeout: 10000, // Add timeout for better error handling
    })

    // Request interceptor - attach the auth token
    this.client.interceptors.request.use((config) => {
      const token = localStorage.getItem('atomo_auth_token')
      if (token) {
        config.headers.Authorization = `Bearer ${token}`
      }
      return config
    })

    // Response interceptor - unified error handling
    this.client.interceptors.response.use(
      (response) => response,
      (error) => {
        // Global 401 handling: clear the token and bounce to login. Skip this for
        // /auth/me so the mount-time session check (App.tsx) can handle an invalid
        // token in-app (render <Login/>) instead of a hard page redirect.
        const url: string = error.config?.url ?? ''
        if (error.response?.status === 401 && !url.includes('/auth/me')) {
          localStorage.removeItem('atomo_auth_token')
          // Base-aware so it stays under /admin when served from a subpath.
          window.location.href = `${(import.meta as any).env.BASE_URL}login`
        }
        return Promise.reject(error)
      }
    )
  }

  /** True when an auth token is stored (the request interceptor attaches it). */
  isAuthenticated(): boolean {
    return !!localStorage.getItem('atomo_auth_token')
  }

  /** Authenticate against POST /auth/login and persist the returned JWT. */
  async login(email: string, password: string): Promise<void> {
    const res = await this.client.post('/auth/login', { email, password })
    const token = res.data?.token
    if (!token) throw new Error('Login response did not include a token')
    localStorage.setItem('atomo_auth_token', token)
  }

  /** Clear the stored token (sign out). */
  logout(): void {
    localStorage.removeItem('atomo_auth_token')
  }

  /** The resolved API base URL (empty string = same-origin). For the Settings page. */
  getBaseUrl(): string {
    return this.baseUrl
  }

  /** Build/version info from GET /version (commit + build time of the running server). */
  async getVersion(): Promise<{ name?: string; version?: string; commit?: string; buildTime?: string }> {
    const res = await this.client.get('/version')
    return res.data
  }

  /**
   * Validate the stored token and fetch the current user from GET /auth/me.
   * Resolves with the user when the token is valid; rejects (401) when it isn't —
   * the caller (App mount check) clears the token and shows the login form.
   */
  async getCurrentUser(): Promise<AuthUser> {
    const res = await this.client.get('/auth/me')
    return res.data as AuthUser
  }

  /**
   * Simplified and more reliable API base URL detection
   */
  private getApiBaseUrl(): string {
    // Check for explicit environment variable first
    const envUrl = (import.meta as any).env?.VITE_API_URL
    if (envUrl) {
      return envUrl
    }

    // In development, try common backend ports
    if ((import.meta as any).env?.DEV) {
      const currentHost = window.location.hostname
      // Try CRM service port first (most common)
      return `http://${currentHost}:3000`
    }

    // In production the SPA is served same-origin (e.g. by atomo-server at
    // /admin), so call the API at the root of the current origin: '' yields
    // root-relative URLs like /graphql and /media, matching the server routes.
    return ''
  }

  /**
   * Get the service's schema metadata.
   */
  async getSchemaMetadata(): Promise<SchemaMetadata> {
    return loadSchemaMetadata()
  }

  /**
   * GraphQL query with better error handling.
   */
  async graphql(query: string, variables?: Record<string, any>): Promise<any> {
    try {
      const response = await this.client.post('/graphql', {
        query,
        variables,
      })

      if (response.data.errors) {
        console.error('GraphQL errors:', response.data.errors)
        throw new Error(response.data.errors[0].message)
      }

      return response.data.data
    } catch (error: any) {
      console.error('GraphQL request failed:', error)
      // If it's a network error and we're in development, provide helpful message
      if (error.code === 'ECONNREFUSED' && (import.meta as any).env?.DEV) {
        throw new Error(`Cannot connect to API server. Make sure the backend service is running on ${this.baseUrl}`)
      }
      throw error
    }
  }

  /**
   * List query - supports pagination, sorting, and filtering.
   */
  async listEntities(modelName: string, options: QueryOptions = {}): Promise<{
    data: EntityData[]
    total: number
    page: number
    limit: number
  }> {
    const { page = 1, limit = 20, sort, order = 'desc', filters = {} } = options
    const offset = (page - 1) * limit

    // Build where filter
    const where_: Record<string, any> = {}
    for (const [key, value] of Object.entries(filters)) {
      if (value !== undefined && value !== '') {
        where_[key] = typeof value === 'string' ? { contains: value } : { equals: value }
      }
    }

    const orderBy = sort ? { [sort]: order.toUpperCase() } : undefined

    let result: any
    try {
      result = await this.graphql(`
        query($model: String!, $where: JSON, $orderBy: JSON, $limit: Int, $offset: Int) {
          paginatedRecords(model: $model, where: $where, orderBy: $orderBy, limit: $limit, offset: $offset) {
            data
            pageInfo { totalCount hasNextPage hasPreviousPage }
          }
        }
      `, {
        model: modelName,
        where: Object.keys(where_).length ? where_ : undefined,
        orderBy,
        limit,
        offset,
      })
    } catch (error) {
      if (this.canUseDemoData()) {
        this.usedDemoData = true
        return this.listDemoEntities(modelName, options)
      }
      throw error
    }

    const paginated = result.paginatedRecords
    return {
      data: paginated?.data || [],
      total: paginated?.pageInfo?.totalCount || 0,
      page,
      limit,
    }
  }

  /**
   * Get a single entity.
   */
  async getEntity(modelName: string, id: string): Promise<EntityData> {
    let result: any
    try {
      result = await this.graphql(`
        query($model: String!, $id: String!) {
          record(model: $model, id: $id)
        }
      `, { model: modelName, id })
    } catch (error) {
      if (this.canUseDemoData()) {
        this.usedDemoData = true
        const entity = this.getDemoEntities(modelName).find((item) => item.id === id)
        if (entity) return entity
      }
      throw error
    }
    return result.record
  }

  /**
   * Create an entity.
   */
  async createEntity(modelName: string, data: Record<string, any>): Promise<EntityData> {
    let result: any
    try {
      result = await this.graphql(`
        mutation($model: String!, $data: JSON!) {
          create(model: $model, data: $data)
        }
      `, { model: modelName, data })
    } catch (error) {
      if (this.canUseDemoData()) {
        this.usedDemoData = true
        const entity = {
          id: `${modelName.toLowerCase()}_${Date.now()}`,
          ...data,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        } as EntityData
        this.getDemoEntities(modelName).push(entity)
        return entity
      }
      throw error
    }
    return result.create
  }

  /**
   * Update an entity.
   */
  async updateEntity(modelName: string, id: string, data: Record<string, any>): Promise<EntityData> {
    let result: any
    try {
      result = await this.graphql(`
        mutation($model: String!, $where: JSON!, $data: JSON!) {
          update(model: $model, where: $where, data: $data)
        }
      `, { model: modelName, where: { id: { equals: id } }, data })
    } catch (error) {
      if (this.canUseDemoData()) {
        this.usedDemoData = true
        const entities = this.getDemoEntities(modelName)
        const index = entities.findIndex((item) => item.id === id)
        if (index >= 0) {
          entities[index] = {
            ...entities[index],
            ...data,
            updatedAt: new Date().toISOString(),
          }
          return entities[index]
        }
      }
      throw error
    }
    return result.update
  }

  /**
   * Delete an entity.
   */
  async deleteEntity(modelName: string, id: string): Promise<void> {
    try {
      await this.graphql(`
        mutation($model: String!, $where: JSON!) {
          delete(model: $model, where: $where)
        }
      `, { model: modelName, where: { id: { equals: id } } })
    } catch (error) {
      if (this.canUseDemoData()) {
        this.usedDemoData = true
        const entities = this.getDemoEntities(modelName)
        const index = entities.findIndex((item) => item.id === id)
        if (index >= 0) {
          entities.splice(index, 1)
          return
        }
      }
      throw error
    }
  }

  // ── Trash / soft-delete management ────────────────────────────────
  /** List soft-deleted records for a model (the trash view). */
  async listDeleted(modelName: string, limit = 50, offset = 0): Promise<EntityData[]> {
    const result = await this.graphql(`
      query($model: String!, $limit: Int, $offset: Int) {
        deletedRecords(model: $model, limit: $limit, offset: $offset) {
          data
          pageInfo { totalCount }
        }
      }
    `, { model: modelName, limit, offset })
    return result.deletedRecords?.data || []
  }

  /** Restore a soft-deleted record by id. */
  async restoreEntity(modelName: string, id: string): Promise<void> {
    await this.graphql(`
      mutation($model: String!, $where: JSON!) {
        restore(model: $model, where: $where)
      }
    `, { model: modelName, where: { id: { equals: id } } })
  }

  /** Permanently delete (purge) a record by id. */
  async hardDeleteEntity(modelName: string, id: string): Promise<void> {
    await this.graphql(`
      mutation($model: String!, $where: JSON!) {
        hardDelete(model: $model, where: $where)
      }
    `, { model: modelName, where: { id: { equals: id } } })
  }

  /**
   * Bulk operations.
   */
  async bulkDelete(modelName: string, ids: string[]): Promise<number> {
    const query = `
      mutation BulkDelete${modelName}($where: CompanyBoolExp!) {
        delete${modelName}(where: $where) {
          affectedRows
        }
      }
    `

    const result = await this.graphql(query, { where: { id: { _in: ids } } })
    return result[`affectedRows`]
  }

  /**
   * REST fallback methods for basic operations (inspired by CRM service)
   */
  async getEntityRest(entityType: string, id: string): Promise<any> {
    try {
      const response = await this.client.get(`/${entityType.toLowerCase()}s/${id}`)
      return response.data
    } catch (error) {
      console.error(`Failed to fetch ${entityType}:`, error)
      throw error
    }
  }

  async listEntitiesRest(entityType: string, options: {
    filters?: Record<string, any>
    sort?: string
    order?: 'asc' | 'desc'
    limit?: number
    offset?: number
  } = {}): Promise<{ data: any[]; total: number }> {
    try {
      const params = new URLSearchParams()
      if (options.filters) {
        params.append('filters', JSON.stringify(options.filters))
      }
      if (options.sort) params.append('sort', options.sort)
      if (options.order) params.append('order', options.order)
      if (options.limit) params.append('limit', options.limit.toString())
      if (options.offset) params.append('offset', options.offset.toString())

      const response = await this.client.get(`/${entityType.toLowerCase()}s?${params}`)
      return response.data
    } catch (error) {
      console.error(`Failed to fetch ${entityType}s:`, error)
      throw error
    }
  }

  async createEntityRest(entityType: string, data: any): Promise<any> {
    try {
      const response = await this.client.post(`/${entityType.toLowerCase()}s`, data)
      return response.data
    } catch (error) {
      console.error(`Failed to create ${entityType}:`, error)
      throw error
    }
  }

  async updateEntityRest(entityType: string, id: string, data: any): Promise<any> {
    try {
      const response = await this.client.put(`/${entityType.toLowerCase()}s/${id}`, data)
      return response.data
    } catch (error) {
      console.error(`Failed to update ${entityType}:`, error)
      throw error
    }
  }

  /** Absolute URL to serve a media object by id (e.g. for <img src>). */
  getMediaUrl(id: string): string {
    return `${this.baseUrl}/media/${id}`
  }

  /** Upload a file to POST /media (auth token attached). Returns the new media id + absolute url. */
  async uploadMedia(
    file: File,
    onProgress?: (percent: number) => void
  ): Promise<{ id: string; url: string }> {
    const token = localStorage.getItem('atomo_auth_token')
    const form = new FormData()
    form.append('file', file)
    return new Promise((resolve, reject) => {
      const xhr = new XMLHttpRequest()
      xhr.open('POST', `${this.baseUrl}/media`)
      if (token) xhr.setRequestHeader('Authorization', `Bearer ${token}`)
      xhr.upload.addEventListener('progress', (e) => {
        if (e.lengthComputable && onProgress) onProgress(Math.round((e.loaded / e.total) * 100))
      })
      xhr.addEventListener('load', () => {
        if (xhr.status >= 200 && xhr.status < 300) {
          try {
            const res = JSON.parse(xhr.responseText)
            resolve({ id: res.id, url: this.getMediaUrl(res.id) })
          } catch {
            reject(new Error('invalid upload response'))
          }
        } else {
          reject(new Error(`upload failed: ${xhr.status}`))
        }
      })
      xhr.addEventListener('error', () => reject(new Error('network error')))
      xhr.send(form)
    })
  }

  // Extend method to allow extending the client with custom methods
  extend<T>(methods: T): AtomoApiClient & T {
    return Object.assign(this, methods)
  }

  private canUseDemoData(): boolean {
    return Boolean((import.meta as any).env?.DEV)
  }

  private getDemoEntities(modelName: string): EntityData[] {
    if (!this.demoStore[modelName]) {
      this.demoStore[modelName] = cloneDemoEntities(modelName)
    }
    return this.demoStore[modelName]
  }

  private listDemoEntities(modelName: string, options: QueryOptions = {}) {
    const { page = 1, limit = 20, sort = 'createdAt', order = 'desc', filters = {}, search } = options
    let data = [...this.getDemoEntities(modelName)]

    for (const [key, value] of Object.entries(filters)) {
      if (value !== undefined && value !== '') {
        data = data.filter((entity) => entity[key] === value)
      }
    }

    if (search) {
      const term = search.toLowerCase()
      data = data.filter((entity) =>
        Object.values(entity).some((value) => String(value).toLowerCase().includes(term))
      )
    }

    data.sort((a, b) => {
      const left = a[sort]
      const right = b[sort]
      if (left === right) return 0
      const comparison = left > right ? 1 : -1
      return order === 'asc' ? comparison : -comparison
    })

    const total = data.length
    const offset = (page - 1) * limit
    return {
      data: data.slice(offset, offset + limit),
      total,
      page,
      limit,
    }
  }

  // ── Workflows (REST) ──────────────────────────────────────────────
  /** List registered workflow names. */
  async listWorkflows(): Promise<string[]> {
    const response = await this.client.get('/workflows')
    return response.data
  }

  /** Fetch a single workflow definition by name. */
  async getWorkflow(name: string): Promise<any> {
    const response = await this.client.get(`/workflows/${encodeURIComponent(name)}`)
    return response.data
  }

  /** Register a workflow definition. */
  async registerWorkflow(workflow: any): Promise<{ registered: string }> {
    const response = await this.client.post('/workflows', workflow)
    return response.data
  }

  /** Run a workflow by name with an optional context. */
  async runWorkflow(name: string, context: Record<string, any> = {}): Promise<any> {
    const response = await this.client.post(`/workflows/${encodeURIComponent(name)}/run`, context)
    return response.data
  }

  /** Remove a registered workflow by name. */
  async deleteWorkflow(name: string): Promise<void> {
    await this.client.delete(`/workflows/${encodeURIComponent(name)}`)
  }
}

// Export the singleton instance
export const apiClient = new AtomoApiClient()
export default apiClient
