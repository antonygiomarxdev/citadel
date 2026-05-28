import * as fs from 'fs';
import type { Language, ExtractionResult } from '../types';

/**
 * Result from a single pipeline execution.
 * Mirrors IndexResult but is pipeline-scoped, not orchestrator-scoped.
 */
export interface PipelineResult {
  nodesCreated: number;
  edgesCreated: number;
  filesIndexed: number;
  filesSkipped: number;
  filesErrored: number;
  errors: { message: string; filePath?: string; severity: string; code?: string }[];
}

export interface IndexProgress {
  phase: 'scanning' | 'parsing' | 'storing' | 'resolving';
  current: number;
  total: number;
  currentFile?: string;
}

/**
 * Persistence layer for extraction results.
 * Implementations write nodes, edges, and file records to a backend.
 */
export interface ResultStore {
  storeResult(
    filePath: string,
    contentHash: string,
    language: Language,
    stats: fs.Stats,
    result: ExtractionResult
  ): void;

  storeExtractionResult(
    filePath: string,
    content: string,
    language: Language,
    stats: fs.Stats,
    result: ExtractionResult
  ): void;
}

/**
 * A parse pipeline processes a list of files and stores results.
 * Implementations differ in strategy: native (Rust thread-local parsers) vs WASM.
 */
export interface ParsePipeline {
  execute(
    files: string[],
    rootDir: string,
    frameworkNames: string[],
    onProgress?: (progress: IndexProgress) => void,
    signal?: AbortSignal
  ): Promise<PipelineResult>;
}
