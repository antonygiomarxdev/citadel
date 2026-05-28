import * as fs from 'fs';
import type { QueryBuilder } from '../db/queries';
import type { Language, ExtractionResult, FileRecord } from '../types';
import { hashContent } from '../utils/hash';
import type { ResultStore } from './interfaces';

export class SqliteResultStore implements ResultStore {
  constructor(private queries: QueryBuilder) {}

  storeResult(
    filePath: string,
    contentHash: string,
    language: Language,
    stats: fs.Stats,
    result: ExtractionResult
  ): void {
    const existingFile = this.queries.getFileByPath(filePath);
    if (existingFile && existingFile.contentHash === contentHash) {
      return;
    }

    if (existingFile) {
      this.queries.deleteFile(filePath);
    }

    const validNodes = result.nodes.filter((n) => n.id && n.kind && n.name && n.filePath && n.language);

    if (validNodes.length > 0) {
      this.queries.insertNodes(validNodes);
    }

    if (result.edges.length > 0) {
      const insertedIds = new Set(validNodes.map((n) => n.id));
      const validEdges = result.edges.filter(
        (e) => insertedIds.has(e.source) && insertedIds.has(e.target)
      );
      if (validEdges.length > 0) {
        this.queries.insertEdges(validEdges);
      }
    }

    if (result.unresolvedReferences.length > 0) {
      const insertedIds = new Set(validNodes.map((n) => n.id));
      const refsWithContext = result.unresolvedReferences
        .filter((ref) => insertedIds.has(ref.fromNodeId))
        .map((ref) => ({
          ...ref,
          filePath: ref.filePath ?? filePath,
          language: ref.language ?? language,
        }));
      if (refsWithContext.length > 0) {
        this.queries.insertUnresolvedRefsBatch(refsWithContext);
      }
    }

    const fileRecord: FileRecord = {
      path: filePath,
      contentHash,
      language,
      size: stats.size,
      modifiedAt: stats.mtimeMs,
      indexedAt: Date.now(),
      nodeCount: result.nodes.length,
      errors: result.errors.length > 0 ? result.errors : undefined,
    };
    this.queries.upsertFile(fileRecord);
  }

  storeExtractionResult(
    filePath: string,
    content: string,
    language: Language,
    stats: fs.Stats,
    result: ExtractionResult
  ): void {
    this.storeResult(filePath, hashContent(content), language, stats, result);
  }
}

/**
 * In-memory result store for testing.
 * Accumulates results in arrays so tests can inspect what would have been persisted.
 */
export class MemoryResultStore implements ResultStore {
  results: Array<{
    filePath: string;
    contentHash?: string;
    content?: string;
    language: Language;
    stats: fs.Stats;
    result: ExtractionResult;
  }> = [];

  storeResult(
    filePath: string,
    contentHash: string,
    language: Language,
    stats: fs.Stats,
    result: ExtractionResult
  ): void {
    this.results.push({ filePath, contentHash, language, stats, result });
  }

  storeExtractionResult(
    filePath: string,
    content: string,
    language: Language,
    stats: fs.Stats,
    result: ExtractionResult
  ): void {
    this.results.push({ filePath, content, language, stats, result });
  }

  clear(): void {
    this.results = [];
  }
}
