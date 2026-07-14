import { describe, it, expect } from 'vitest'
import { canPerform } from './permissions'

const model = (rule: string | null | undefined) => ({ access: { create: rule } }) as any

describe('canPerform', () => {
  it('fails open when no rule is declared', () => {
    expect(canPerform({ access: undefined } as any, 'create', 'viewer')).toBe(true)
    expect(canPerform(model(undefined), 'create', 'viewer')).toBe(true)
    expect(canPerform(model(null), 'create', 'viewer')).toBe(true)
  })

  it('hides server-only and never rules from every human', () => {
    expect(canPerform(model('system'), 'create', 'admin')).toBe(false)
    expect(canPerform(model('never'), 'create', 'admin')).toBe(false)
  })

  it('gates pipe-joined role lists case-insensitively', () => {
    expect(canPerform(model('admin|manager'), 'create', 'Admin')).toBe(true)
    expect(canPerform(model('admin|manager'), 'create', 'sales')).toBe(false)
    expect(canPerform(model('admin|manager'), 'create', null)).toBe(false)
  })

  it('authenticated allows any signed-in role', () => {
    expect(canPerform(model('authenticated'), 'create', 'viewer')).toBe(true)
    expect(canPerform(model('authenticated'), 'create', null)).toBe(false)
  })

  it('owner rules stay visible — only the server can decide per record', () => {
    expect(canPerform(model('owner'), 'create', 'viewer')).toBe(true)
    expect(canPerform(model('owner|admin'), 'create', 'viewer')).toBe(true)
  })

  it('public allows everyone', () => {
    expect(canPerform(model('public'), 'create', null)).toBe(true)
  })
})
