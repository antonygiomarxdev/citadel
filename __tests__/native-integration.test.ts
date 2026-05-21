import { describe, it, expect } from 'vitest'
import { resolve } from 'path'
import { readdirSync } from 'fs'

function loadNative() {
  const dist = resolve(__dirname, '..', 'dist')
  const nodeFile = readdirSync(dist).find(f => f.endsWith('.node') && !f.endsWith('.node.js'))
  if (!nodeFile) throw new Error('No .node file found in dist/')
  return require(resolve(dist, nodeFile))
}

describe('native bindings', () => {
  it('should load and expose getVersion', () => {
    const native = loadNative()
    expect(native.getVersion).toBeDefined()
    expect(typeof native.getVersion).toBe('function')
  })

  it('getVersion should return a semver string', () => {
    const native = loadNative()
    const v = native.getVersion()
    expect(v).toMatch(/^\d+\.\d+\.\d+/)
  })
})
