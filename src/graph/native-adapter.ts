/**
 * Native Graph Adapter
 *
 * Bridges the TypeScript codebase to the Rust NAPI `Database` class
 * for graph traversal operations. Replaces the TypeScript `GraphTraverser`
 * (642 lines of duplicated BFS/DFS/callers/callees/etc.) with direct
 * calls to the Rust `GraphQuery` blanket impl.
 *
 * All methods accept/return JSON strings because the NAPI boundary
 * serializes Rust structs <-> JSON. This adapter translates between
 * the Rust types and the TypeScript Subgraph/Node/Edge types.
 */

import type { Node, Edge, Subgraph, TraversalOptions, EdgeKind } from '../types';

type NativeNode = Node;
type NativeEdge = Edge;

/**
 * Parse JSON safely, returning a fallback on error.
 */
function safeParse<T>(json: string, fallback: T): T {
  try {
    if (!json || json === 'null') return fallback;
    return JSON.parse(json) as T;
  } catch {
    return fallback;
  }
}

/**
 * Convert Rust BFS/DFS `Vec<(Node, Vec<Edge>)>` to a TS Subgraph.
 */
function traversalResultToSubgraph(
  results: [NativeNode, NativeEdge[]][],
  startId: string
): Subgraph {
  const nodes = new Map<string, Node>();
  const edges: Edge[] = [];
  for (const [node, nodeEdges] of results) {
    nodes.set(node.id, node);
    for (const edge of nodeEdges) {
      edges.push(edge);
    }
  }
  return { nodes, edges, roots: [startId] };
}

/**
 * Convert callers/callees `Vec<(Node, Edge)>` to result format.
 */
function pairsToResult(
  results: [NativeNode, NativeEdge][]
): Array<{ node: Node; edge: Edge }> {
  return results.map(([node, edge]) => ({ node, edge }));
}

/**
 * Convert `Vec<(Node, Vec<Edge>)>` to flat { nodes, edges } format.
 */
function traversalResultToFlat(
  results: [NativeNode, NativeEdge[]][]
): { nodes: Node[]; edges: Edge[] } {
  const nodeMap = new Map<string, Node>();
  const edges: Edge[] = [];
  for (const [node, nodeEdges] of results) {
    nodeMap.set(node.id, node);
    for (const edge of nodeEdges) {
      edges.push(edge);
    }
  }
  return { nodes: Array.from(nodeMap.values()), edges };
}

/**
 * NativeDatabase wraps the NAPI Database class.
 */
class NativeDatabase {
  private static instance: any = null;

  static getInstance(): any {
    if (!NativeDatabase.instance) {
      try {
        const mod = require('../../dist/citadel-native.linux-x64-gnu.node');
        NativeDatabase.instance = new mod.Database();
      } catch (e) {
        throw new Error(
          `Failed to load native Database module. Make sure citadel-native is built for your platform: ${e}`
        );
      }
    }
    return NativeDatabase.instance;
  }
}

/**
 * NativeGraphAdapter — drop-in replacement for the TypeScript GraphTraverser.
 */
export class NativeGraphAdapter {
  private db: any;
  private initialized = false;

  constructor(dbPath: string) {
    this.db = NativeDatabase.getInstance();
    this.db.open(dbPath);
    this.initialized = true;
  }

  /**
   * Clean up the native connection. Call when done.
   */
  close(): void {
    if (this.initialized) {
      try { this.db.close(); } catch { /* ignore */ }
      this.initialized = false;
    }
  }

  // ── Traversal ──

  traverseBFS(startId: string, options: TraversalOptions = {}): Subgraph {
    const raw = this.db.traverseBfs(startId, JSON.stringify(options));
    const results = safeParse<[NativeNode, NativeEdge[]][]>(raw, []);
    return traversalResultToSubgraph(results, startId);
  }

  traverseDFS(startId: string, options: TraversalOptions = {}): Subgraph {
    const raw = this.db.traverseDfs(startId, JSON.stringify(options));
    const results = safeParse<[NativeNode, NativeEdge[]][]>(raw, []);
    return traversalResultToSubgraph(results, startId);
  }

  getCallers(nodeId: string, maxDepth: number = 1): Array<{ node: Node; edge: Edge }> {
    const raw = this.db.getCallers(nodeId, maxDepth);
    const results = safeParse<[NativeNode, NativeEdge][]>(raw, []);
    return pairsToResult(results);
  }

  getCallees(nodeId: string, maxDepth: number = 1): Array<{ node: Node; edge: Edge }> {
    const raw = this.db.getCallees(nodeId, maxDepth);
    const results = safeParse<[NativeNode, NativeEdge][]>(raw, []);
    return pairsToResult(results);
  }

  getCallGraph(nodeId: string, depth: number = 2): Subgraph {
    const raw = this.db.getCallGraph(nodeId, depth);
    const results = safeParse<[NativeNode, NativeEdge[]][]>(raw, []);
    const subgraph = traversalResultToFlat(results);
    const nodes = new Map<string, Node>();
    for (const n of subgraph.nodes) nodes.set(n.id, n);
    return { nodes, edges: subgraph.edges, roots: [nodeId] };
  }

  getTypeHierarchy(nodeId: string): Subgraph {
    const raw = this.db.getTypeHierarchy(nodeId);
    const results = safeParse<[NativeNode, NativeEdge[]][]>(raw, []);
    const subgraph = traversalResultToFlat(results);
    const nodes = new Map<string, Node>();
    for (const n of subgraph.nodes) nodes.set(n.id, n);
    return { nodes, edges: subgraph.edges, roots: [nodeId] };
  }

  findUsages(nodeId: string): Array<{ node: Node; edge: Edge }> {
    const raw = this.db.findUsages(nodeId);
    const results = safeParse<[NativeNode, NativeEdge][]>(raw, []);
    return pairsToResult(results);
  }

  getImpactRadius(nodeId: string, maxDepth: number = 3): Subgraph {
    const raw = this.db.getImpactRadius(nodeId, maxDepth);
    const results = safeParse<[NativeNode, NativeEdge[]][]>(raw, []);
    const subgraph = traversalResultToFlat(results);
    const nodes = new Map<string, Node>();
    for (const n of subgraph.nodes) nodes.set(n.id, n);
    return { nodes, edges: subgraph.edges, roots: [nodeId] };
  }

  findPath(
    fromId: string,
    toId: string,
    edgeKinds?: EdgeKind[]
  ): Array<{ node: Node; edge: Edge | null }> | null {
    const kindsJson = edgeKinds ? JSON.stringify(edgeKinds) : '';
    const raw: string | null = this.db.findShortestPath(fromId, toId, kindsJson);
    if (!raw) return null;
    const steps = safeParse<[NativeNode, NativeEdge][]>(raw, []);
    if (steps.length === 0) return null;
    return steps.map(([node, edge], i) => ({
      node,
      edge: i === 0 ? null : edge,
    }));
  }

  getAncestors(nodeId: string): Node[] {
    const raw = this.db.getAncestors(nodeId);
    return safeParse<Node[]>(raw, []);
  }

  getChildren(nodeId: string): Node[] {
    const raw = this.db.getChildren(nodeId);
    return safeParse<Node[]>(raw, []);
  }
}
