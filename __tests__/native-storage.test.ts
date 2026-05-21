import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { resolve } from 'path'
import { readdirSync, mkdtempSync, existsSync, rmSync } from 'fs'
import { tmpdir } from 'os'
import { join } from 'path'

function loadNative() {
  const dist = resolve(__dirname, '..', 'dist')
  const nodeFile = readdirSync(dist).find(f => f.endsWith('.node') && !f.endsWith('.node.js'))
  if (!nodeFile) throw new Error('No .node file found in dist/')
  return require(resolve(dist, nodeFile))
}

type Database = import('../dist/native').Database

interface Node {
  id: string
  kind: string
  name: string
  qualified_name: string
  file_path: string
  language: string
  start_line: number
  end_line: number
  start_column: number
  end_column: number
  docstring: string | null
  signature: string | null
  visibility: string | null
  is_exported: boolean
  is_async: boolean
  is_static: boolean
  is_abstract: boolean
  decorators: string[] | null
  type_parameters: string[] | null
  updated_at: number
}

const makeNode = (overrides: Partial<Node> = {}): Node => ({
  id: 'n1',
  kind: 'function',
  name: 'hello',
  qualified_name: 'hello',
  file_path: 'main.ts',
  language: 'typescript',
  start_line: 1,
  end_line: 5,
  start_column: 0,
  end_column: 0,
  docstring: null,
  signature: 'fn hello()',
  visibility: null,
  is_exported: true,
  is_async: false,
  is_static: false,
  is_abstract: false,
  decorators: null,
  type_parameters: null,
  updated_at: 12345,
  ...overrides,
})

interface Edge {
  source: string
  target: string
  kind: string
  metadata: Record<string, unknown> | null
  line: number | null
  column: number | null
  provenance: string | null
}

const makeEdge = (overrides: Partial<Edge> = {}): Edge => ({
  source: 'n1',
  target: 'n2',
  kind: 'calls',
  metadata: null,
  line: null,
  column: null,
  provenance: null,
  ...overrides,
})

interface FileRecord {
  path: string
  content_hash: string
  language: string
  size: number
  modified_at: number
  indexed_at: number
  node_count: number
  errors: unknown[] | null
}

const makeFile = (overrides: Partial<FileRecord> = {}): FileRecord => ({
  path: 'main.ts',
  content_hash: 'abc123',
  language: 'typescript',
  size: 100,
  modified_at: 12345,
  indexed_at: 12345,
  node_count: 1,
  errors: null,
  ...overrides,
})

interface SearchResult {
  node: Node
  score: number
  highlights: string[] | null
}

interface GraphStats {
  node_count: number
  edge_count: number
  file_count: number
  nodes_by_kind: Record<string, number>
  edges_by_kind: Record<string, number>
  files_by_language: Record<string, number>
  db_size_bytes: number
  last_updated: number
}

describe('native Database', () => {
  const native = loadNative()
  let db: Database
  let tmpDir: string

  beforeEach(() => {
    tmpDir = mkdtempSync(join(tmpdir(), 'cg-native-test-'))
    db = new native.Database()
  })

  afterEach(() => {
    try { db.close() } catch { /* ignore */ }
    if (tmpDir && existsSync(tmpDir)) rmSync(tmpDir, { recursive: true, force: true })
  })

  it('should construct a Database', () => {
    expect(db).toBeDefined()
    expect(typeof db.initialize).toBe('function')
    expect(typeof db.open).toBe('function')
    expect(typeof db.close).toBe('function')
  })

  it('should initialize and create a database file', () => {
    const dbPath = join(tmpDir, 'test.db')
    expect(existsSync(dbPath)).toBe(false)
    db.initialize(dbPath)
    expect(existsSync(dbPath)).toBe(true)
    expect(db.isOpen()).toBe(true)
  })

  it('should open an existing database', () => {
    const dbPath = join(tmpDir, 'test.db')
    db.initialize(dbPath)
    db.close()
    expect(db.isOpen()).toBe(false)

    const db2 = new native.Database()
    db2.open(dbPath)
    expect(db2.isOpen()).toBe(true)
    db2.close()
  })

  describe('node CRUD', () => {
    it('should insert and retrieve a node by id', () => {
      const dbPath = join(tmpDir, 'test.db')
      db.initialize(dbPath)

      db.insertNode(JSON.stringify(makeNode()))

      const result = db.getNodeById('n1')
      expect(result).not.toBeNull()
      const node: Node = JSON.parse(result!)
      expect(node.name).toBe('hello')
      expect(node.kind).toBe('function')
      expect(node.signature).toBe('fn hello()')
    })

    it('should insert multiple nodes', () => {
      db.initialize(join(tmpDir, 'test.db'))

      const nodes = [
        makeNode({ id: 'n1', name: 'foo', kind: 'function' }),
        makeNode({ id: 'n2', name: 'BarClass', kind: 'class' }),
        makeNode({ id: 'n3', name: 'baz', kind: 'variable' }),
      ]
      db.insertNodes(JSON.stringify(nodes))

      const allJson = db.getAllNodes()
      const all: Node[] = JSON.parse(allJson)
      expect(all).toHaveLength(3)
    })

    it('should return null for missing node', () => {
      db.initialize(join(tmpDir, 'test.db'))
      expect(db.getNodeById('nonexistent')).toBeNull()
    })

    it('should get nodes by file', () => {
      db.initialize(join(tmpDir, 'test.db'))
      db.insertNode(JSON.stringify(makeNode()))
      db.insertNode(JSON.stringify(makeNode({ id: 'n2', name: 'world', file_path: 'other.ts' })))

      const mainNodes = JSON.parse(db.getNodesByFile('main.ts'))
      expect(mainNodes).toHaveLength(1)
      const otherNodes = JSON.parse(db.getNodesByFile('other.ts'))
      expect(otherNodes).toHaveLength(1)
    })

    it('should get nodes by kind', () => {
      db.initialize(join(tmpDir, 'test.db'))
      db.insertNodes(JSON.stringify([
        makeNode({ id: 'n1', kind: 'function', name: 'foo' }),
        makeNode({ id: 'n2', kind: 'class', name: 'Foo' }),
        makeNode({ id: 'n3', kind: 'function', name: 'bar' }),
      ]))

      const funcs = JSON.parse(db.getNodesByKind('function'))
      expect(funcs).toHaveLength(2)
      const classes = JSON.parse(db.getNodesByKind('class'))
      expect(classes).toHaveLength(1)
    })

    it('should update a node', () => {
      db.initialize(join(tmpDir, 'test.db'))
      db.insertNode(JSON.stringify(makeNode()))

      db.updateNode(JSON.stringify(makeNode({ signature: 'fn updated()' })))

      const updated = JSON.parse(db.getNodeById('n1')!)
      expect(updated.signature).toBe('fn updated()')
    })

    it('should delete a node', () => {
      db.initialize(join(tmpDir, 'test.db'))
      db.insertNode(JSON.stringify(makeNode()))
      expect(JSON.parse(db.getNodeById('n1')!)).toBeTruthy()

      db.deleteNode('n1')
      expect(db.getNodeById('n1')).toBeNull()
    })

    it('should delete nodes by file', () => {
      db.initialize(join(tmpDir, 'test.db'))
      db.insertNodes(JSON.stringify([
        makeNode({ id: 'n1', file_path: 'main.ts' }),
        makeNode({ id: 'n2', file_path: 'main.ts' }),
        makeNode({ id: 'n3', file_path: 'other.ts' }),
      ]))

      db.deleteNodesByFile('main.ts')
      expect(JSON.parse(db.getAllNodes())).toHaveLength(1)
    })

    it('should get nodes by name', () => {
      db.initialize(join(tmpDir, 'test.db'))
      db.insertNodes(JSON.stringify([
        makeNode({ id: 'n1', name: 'hello' }),
        makeNode({ id: 'n2', name: 'world' }),
      ]))

      const results = JSON.parse(db.getNodesByName('hello'))
      expect(results).toHaveLength(1)
      expect(results[0].name).toBe('hello')
    })
  })

  describe('edge CRUD', () => {
    beforeEach(() => {
      db.initialize(join(tmpDir, 'test.db'))
      db.insertNodes(JSON.stringify([
        makeNode({ id: 'n1', name: 'caller' }),
        makeNode({ id: 'n2', name: 'callee' }),
        makeNode({ id: 'n3', name: 'other' }),
      ]))
    })

    it('should insert and retrieve outgoing edges', () => {
      db.insertEdge(JSON.stringify(makeEdge({ source: 'n1', target: 'n2', kind: 'calls' })))
      db.insertEdge(JSON.stringify(makeEdge({ source: 'n1', target: 'n3', kind: 'references' })))

      const outgoing = JSON.parse(db.getOutgoingEdges('n1', null, null))
      expect(outgoing).toHaveLength(2)
    })

    it('should filter outgoing edges by kind', () => {
      db.insertEdge(JSON.stringify(makeEdge({ source: 'n1', target: 'n2', kind: 'calls' })))
      db.insertEdge(JSON.stringify(makeEdge({ source: 'n1', target: 'n3', kind: 'references' })))

      const calls = JSON.parse(db.getOutgoingEdges('n1', 'calls', null))
      expect(calls).toHaveLength(1)
      expect(calls[0].kind).toBe('calls')
    })

    it('should retrieve incoming edges', () => {
      db.insertEdge(JSON.stringify(makeEdge({ source: 'n1', target: 'n2', kind: 'calls' })))
      db.insertEdge(JSON.stringify(makeEdge({ source: 'n3', target: 'n2', kind: 'references' })))

      const incoming = JSON.parse(db.getIncomingEdges('n2', null))
      expect(incoming).toHaveLength(2)
    })

    it('should delete edges by source', () => {
      db.insertEdge(JSON.stringify(makeEdge({ source: 'n1', target: 'n2', kind: 'calls' })))
      db.insertEdge(JSON.stringify(makeEdge({ source: 'n1', target: 'n3', kind: 'references' })))
      expect(JSON.parse(db.getOutgoingEdges('n1', null, null))).toHaveLength(2)

      db.deleteEdgesBySource('n1')
      expect(JSON.parse(db.getOutgoingEdges('n1', null, null))).toHaveLength(0)
    })

    it('should insert edges in batch', () => {
      const edges = [
        makeEdge({ source: 'n1', target: 'n2', kind: 'calls' }),
        makeEdge({ source: 'n2', target: 'n3', kind: 'calls' }),
        makeEdge({ source: 'n3', target: 'n1', kind: 'references' }),
      ]
      db.insertEdges(JSON.stringify(edges))

      expect(JSON.parse(db.getOutgoingEdges('n1', null, null))).toHaveLength(1)
      expect(JSON.parse(db.getOutgoingEdges('n2', null, null))).toHaveLength(1)
      expect(JSON.parse(db.getOutgoingEdges('n3', null, null))).toHaveLength(1)
    })
  })

  describe('file CRUD', () => {
    beforeEach(() => {
      db.initialize(join(tmpDir, 'test.db'))
    })

    it('should upsert and retrieve a file', () => {
      db.upsertFile(JSON.stringify(makeFile()))

      const result = db.getFileByPath('main.ts')
      expect(result).not.toBeNull()
      const file: FileRecord = JSON.parse(result!)
      expect(file.path).toBe('main.ts')
      expect(file.content_hash).toBe('abc123')
    })

    it('should return null for missing file', () => {
      expect(db.getFileByPath('nonexistent.ts')).toBeNull()
    })

    it('should list all files', () => {
      db.upsertFile(JSON.stringify(makeFile({ path: 'a.ts', content_hash: 'aaa' })))
      db.upsertFile(JSON.stringify(makeFile({ path: 'b.ts', content_hash: 'bbb' })))

      const files: FileRecord[] = JSON.parse(db.getAllFiles())
      expect(files).toHaveLength(2)
      const paths = files.map(f => f.path).sort()
      expect(paths).toEqual(['a.ts', 'b.ts'])
    })

    it('should delete a file and its nodes', () => {
      db.upsertFile(JSON.stringify(makeFile({ path: 'main.ts' })))
      db.insertNode(JSON.stringify(makeNode({ file_path: 'main.ts' })))

      db.deleteFile('main.ts')
      expect(db.getFileByPath('main.ts')).toBeNull()
      expect(JSON.parse(db.getNodesByFile('main.ts'))).toHaveLength(0)
    })

    it('should get all file paths', () => {
      db.upsertFile(JSON.stringify(makeFile({ path: 'a.ts' })))
      db.upsertFile(JSON.stringify(makeFile({ path: 'b.ts' })))

      const paths = db.getAllFilePaths()
      expect(paths).toEqual(expect.arrayContaining(['a.ts', 'b.ts']))
    })
  })

  describe('search', () => {
    beforeEach(() => {
      db.initialize(join(tmpDir, 'test.db'))
      db.insertNodes(JSON.stringify([
        makeNode({ id: 'n1', name: 'helloWorld', kind: 'function', signature: 'fn helloWorld()' }),
        makeNode({ id: 'n2', name: 'helloYou', kind: 'function', signature: 'fn helloYou()' }),
        makeNode({ id: 'n3', name: 'UserService', kind: 'class', signature: 'class UserService' }),
        makeNode({ id: 'n4', name: 'AuthService', kind: 'class', signature: 'class AuthService' }),
        makeNode({ id: 'n5', name: 'getUser', kind: 'function', signature: 'fn getUser()' }),
        makeNode({ id: 'n6', name: 'setUser', kind: 'function', signature: 'fn setUser()' }),
        makeNode({ id: 'n7', name: 'MAX_RETRIES', kind: 'constant', signature: 'const MAX_RETRIES', language: 'typescript' }),
        makeNode({ id: 'n8', name: 'handleClick', kind: 'function', language: 'typescript' }),
        makeNode({ id: 'n9', name: 'calculateTotal', kind: 'method', language: 'typescript' }),
        makeNode({ id: 'n10', name: 'render', kind: 'method', language: 'tsx' }),
      ]))
    })

    it('should find nodes by name prefix via FTS5', () => {
      const results: SearchResult[] = JSON.parse(db.searchNodes('hello', JSON.stringify({ limit: 10 })))
      expect(results.length).toBeGreaterThanOrEqual(2)
      const names = results.map(r => r.node.name)
      expect(names).toContain('helloWorld')
      expect(names).toContain('helloYou')
    })

    it('should filter by kind', () => {
      const results: SearchResult[] = JSON.parse(
        db.searchNodes('Service', JSON.stringify({ kinds: ['class'], limit: 10 }))
      )
      expect(results.length).toBeGreaterThanOrEqual(2)
      expect(results.every(r => r.node.kind === 'class')).toBe(true)
    })

    it('should filter by language', () => {
      const results: SearchResult[] = JSON.parse(
        db.searchNodes('', JSON.stringify({ languages: ['tsx'], limit: 10 }))
      )
      expect(results).toHaveLength(1)
      expect(results[0].node.name).toBe('render')
    })

    it('should return results with scores', () => {
      const results: SearchResult[] = JSON.parse(db.searchNodes('getUser', JSON.stringify({ limit: 5 })))
      expect(results.length).toBeGreaterThanOrEqual(1)
      expect(results[0].score).toBeTypeOf('number')
    })
  })

  describe('stats and metadata', () => {
    beforeEach(() => {
      db.initialize(join(tmpDir, 'test.db'))
    })

    it('should return stats', () => {
      db.insertNodes(JSON.stringify([
        makeNode({ id: 'n1', kind: 'function', name: 'foo', file_path: 'a.ts' }),
        makeNode({ id: 'n2', kind: 'class', name: 'Foo', file_path: 'a.ts' }),
      ]))
      db.insertEdge(JSON.stringify(makeEdge({ source: 'n1', target: 'n2', kind: 'calls' })))
      db.upsertFile(JSON.stringify(makeFile({ path: 'a.ts', node_count: 2 })))

      const stats: GraphStats = JSON.parse(db.getStats())
      expect(stats.node_count).toBe(2)
      expect(stats.edge_count).toBe(1)
      expect(stats.file_count).toBe(1)
      expect(stats.nodes_by_kind.function).toBe(1)
      expect(stats.nodes_by_kind.class).toBe(1)
    })

    it('should set and get metadata', () => {
      db.setMetadata('key1', 'value1')
      expect(db.getMetadata('key1')).toBe('value1')
      expect(db.getMetadata('nonexistent')).toBeNull()
    })
  })

  describe('clear', () => {
    it('should clear all data', () => {
      db.initialize(join(tmpDir, 'test.db'))
      db.insertNode(JSON.stringify(makeNode({ id: 'n1' })))
      db.insertNode(JSON.stringify(makeNode({ id: 'n2', name: 'world' })))
      db.upsertFile(JSON.stringify(makeFile({ path: 'main.ts' })))
      db.insertEdge(JSON.stringify(makeEdge({ source: 'n1', target: 'n2', kind: 'calls' })))

      db.clear()

      expect(db.getNodeById('n1')).toBeNull()
      expect(db.getFileByPath('main.ts')).toBeNull()
      const stats: GraphStats = JSON.parse(db.getStats())
      expect(stats.node_count).toBe(0)
      expect(stats.edge_count).toBe(0)
      expect(stats.file_count).toBe(0)
    })
  })
})
