/**
 * Graph Traverser Interface
 *
 * Abstraction over graph traversal algorithms. All consumers depend on
 * this interface, allowing the underlying implementation to be swapped:
 *
 * - GraphTraverser   — TypeScript BFS/DFS against QueryBuilder (better-sqlite3)
 * - NativeGraphAdapter — Rust GraphQuery blanket impl via NAPI (rusqlite)
 */

import type { Node, Edge, Subgraph, TraversalOptions, EdgeKind } from '../types';

export interface IGraphTraverser {
  traverseBFS(startId: string, options?: TraversalOptions): Subgraph;
  traverseDFS(startId: string, options?: TraversalOptions): Subgraph;
  getCallers(nodeId: string, maxDepth?: number): Array<{ node: Node; edge: Edge }>;
  getCallees(nodeId: string, maxDepth?: number): Array<{ node: Node; edge: Edge }>;
  getCallGraph(nodeId: string, depth?: number): Subgraph;
  getTypeHierarchy(nodeId: string): Subgraph;
  findUsages(nodeId: string): Array<{ node: Node; edge: Edge }>;
  getImpactRadius(nodeId: string, maxDepth?: number): Subgraph;
  findPath(fromId: string, toId: string, edgeKinds?: EdgeKind[]): Array<{ node: Node; edge: Edge | null }> | null;
  getAncestors(nodeId: string): Node[];
  getChildren(nodeId: string): Node[];
}
