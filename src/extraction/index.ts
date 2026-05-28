import * as fs from 'fs';
import * as path from 'path';
// Languages that have native Rust tree-sitter extractors.
const NATIVE_EXTRACTOR_LANGS = new Set(['typescript', 'javascript', 'tsx', 'jsx', 'python', 'go', 'rust', 'java']);

import { SqliteResultStore } from './result-store';
import { NativePipeline } from './native-pipeline';
import { WasmPipeline } from './wasm-pipeline';
import { SingleFileIndexer } from './single-file-indexer';
import { SyncService } from './sync-service';
import type { IndexProgress } from './interfaces';
import type { ExtractionResult, ExtractionError, CodeGraphConfig } from '../types';
import { QueryBuilder } from '../db/queries';
import { detectLanguage, initGrammars } from './grammars';
import { validatePathWithinRoot } from '../utils';
import { scanDirectory, scanDirectoryAsync } from './file-scanner';
import { detectFrameworks } from '../resolution/frameworks';
import type { ResolutionContext } from '../resolution/types';

export { scanDirectory, scanDirectoryAsync, shouldIncludeFile, getGitVisibleFiles, getGitChangedFiles } from './file-scanner';
export type { GitChanges } from './file-scanner';
export type { PipelineResult, ResultStore, ParsePipeline } from './interfaces';
export type { IndexProgress };

export interface IndexResult {
  success: boolean;
  filesIndexed: number;
  filesSkipped: number;
  filesErrored: number;
  nodesCreated: number;
  edgesCreated: number;
  errors: ExtractionError[];
  durationMs: number;
}

export interface SyncResult {
  filesChecked: number;
  filesAdded: number;
  filesModified: number;
  filesRemoved: number;
  nodesUpdated: number;
  durationMs: number;
  changedFilePaths?: string[];
}

export class ExtractionOrchestrator {
  private detectedFrameworkNames: string[] | null = null;
  private resultStore: SqliteResultStore;
  private fileIndexer: SingleFileIndexer;
  private syncService: SyncService;

  constructor(
    private rootDir: string,
    private config: CodeGraphConfig,
    queries: QueryBuilder,
  ) {
    this.resultStore = new SqliteResultStore(queries);
    this.fileIndexer = new SingleFileIndexer(rootDir, config, this.resultStore);
    this.syncService = new SyncService(rootDir, config, queries, this.fileIndexer);
  }

  private buildDetectionContext(files: string[]): ResolutionContext {
    const rootDir = this.rootDir;
    return {
      getNodesInFile: () => [],
      getNodesByName: () => [],
      getNodesByQualifiedName: () => [],
      getNodesByKind: () => [],
      getNodesByLowerName: () => [],
      getImportMappings: () => [],
      getAllFiles: () => files,
      getProjectRoot: () => rootDir,
      fileExists: (relativePath: string) => {
        const full = validatePathWithinRoot(rootDir, relativePath);
        if (!full) return false;
        try { return fs.existsSync(full); } catch { return false; }
      },
      readFile: (relativePath: string) => {
        const full = validatePathWithinRoot(rootDir, relativePath);
        if (!full) return null;
        try { return fs.readFileSync(full, 'utf-8'); } catch { return null; }
      },
    };
  }

  private ensureDetectedFrameworks(files?: string[]): string[] {
    if (this.detectedFrameworkNames !== null) return this.detectedFrameworkNames;
    const fileList = files ?? scanDirectory(this.rootDir, this.config);
    const context = this.buildDetectionContext(fileList);
    this.detectedFrameworkNames = detectFrameworks(context).map((r) => r.name);
    return this.detectedFrameworkNames;
  }

  async indexAll(onProgress?: (progress: IndexProgress) => void, signal?: AbortSignal): Promise<IndexResult> {
    await initGrammars();
    const startTime = Date.now();
    const errors: ExtractionError[] = [];
    let filesIndexed = 0, filesSkipped = 0, filesErrored = 0, totalNodes = 0, totalEdges = 0;

    onProgress?.({ phase: 'scanning', current: 0, total: 0 });
    const files = await scanDirectoryAsync(this.rootDir, this.config, (current, file) => {
      onProgress?.({ phase: 'scanning', current, total: 0, currentFile: file });
    });

    this.detectedFrameworkNames = null;
    const frameworkNames = this.ensureDetectedFrameworks(files);
    this.fileIndexer.setFrameworkNames(frameworkNames);

    if (signal?.aborted) {
      return { success: false, filesIndexed: 0, filesSkipped: 0, filesErrored: 0, nodesCreated: 0, edgesCreated: 0, errors: [{ message: 'Aborted', severity: 'error' }], durationMs: Date.now() - startTime };
    }

    const total = files.length;
    onProgress?.({ phase: 'parsing', current: 0, total });
    await new Promise(resolve => setImmediate(resolve));

    const wasmPipeline = new WasmPipeline(this.rootDir, this.resultStore, this.config);
    const dbPath = path.join(this.rootDir, '.codegraph', 'codegraph.db');
    const nativePipeline = new NativePipeline(this.rootDir, dbPath, frameworkNames);

    // When the native napi module isn't available (e.g. vitest runs from source),
    // route all files to the WASM pipeline.
    const canUseNative = nativePipeline.available;
    const nativeFiles = canUseNative
      ? files.filter(fp => NATIVE_EXTRACTOR_LANGS.has(detectLanguage(fp)))
      : [];
    const wasmFiles = files.filter(fp => !NATIVE_EXTRACTOR_LANGS.has(detectLanguage(fp)) || !canUseNative);

    let nativeCurrent = 0;
    const [nativeResult, wasmResult] = await Promise.all([
      nativePipeline.execute(nativeFiles, this.rootDir, frameworkNames, (p) => {
        if (p.phase === 'parsing') {
          nativeCurrent = p.current;
          onProgress?.({ phase: 'parsing', current: p.current, total, currentFile: p.currentFile });
        }
      }, signal),
      wasmPipeline.execute(wasmFiles, this.rootDir, frameworkNames, (p) => {
        if (p.phase === 'parsing') onProgress?.({ phase: 'parsing', current: nativeCurrent + p.current, total, currentFile: p.currentFile });
      }, signal),
    ]);

    filesIndexed = nativeResult.filesIndexed + wasmResult.filesIndexed;
    totalNodes = nativeResult.nodesCreated + wasmResult.nodesCreated;
    totalEdges = nativeResult.edgesCreated + wasmResult.edgesCreated;
    filesSkipped = nativeResult.filesSkipped + wasmResult.filesSkipped;
    filesErrored = nativeResult.filesErrored + wasmResult.filesErrored;
    errors.push(...nativeResult.errors.map(e => ({ ...e, severity: e.severity as any })),
                 ...wasmResult.errors.map(e => ({ ...e, severity: e.severity as any })));

    onProgress?.({ phase: 'parsing', current: total, total });
    await new Promise(resolve => setImmediate(resolve));

    return {
      success: filesIndexed > 0 || errors.filter((e) => e.severity === 'error').length === 0,
      filesIndexed, filesSkipped, filesErrored,
      nodesCreated: totalNodes, edgesCreated: totalEdges,
      errors, durationMs: Date.now() - startTime,
    };
  }

  async indexFiles(filePaths: string[]): Promise<IndexResult> {
    const startTime = Date.now();
    const errors: ExtractionError[] = [];
    let filesIndexed = 0, filesSkipped = 0, filesErrored = 0, totalNodes = 0, totalEdges = 0;

    for (const filePath of filePaths) {
      const result = await this.fileIndexer.indexFile(filePath);
      if (result.errors.length > 0) errors.push(...result.errors);
      if (result.nodes.length > 0) { filesIndexed++; totalNodes += result.nodes.length; totalEdges += result.edges.length; }
      else if (result.errors.some((e) => e.severity === 'error')) filesErrored++;
      else filesSkipped++;
    }

    return {
      success: filesIndexed > 0 || errors.filter((e) => e.severity === 'error').length === 0,
      filesIndexed, filesSkipped, filesErrored,
      nodesCreated: totalNodes, edgesCreated: totalEdges,
      errors, durationMs: Date.now() - startTime,
    };
  }

  async indexFile(relativePath: string): Promise<ExtractionResult> {
    return this.fileIndexer.indexFile(relativePath);
  }

  async indexFileWithContent(relativePath: string, content: string, stats: fs.Stats): Promise<ExtractionResult> {
    return this.fileIndexer.indexFileWithContent(relativePath, content, stats);
  }

  async sync(onProgress?: (progress: IndexProgress) => void): Promise<SyncResult> {
    return this.syncService.sync(onProgress);
  }

  getChangedFiles(): { added: string[]; modified: string[]; removed: string[] } {
    return this.syncService.getChangedFiles();
  }
}

export { extractFromSource } from './tree-sitter';
export { detectLanguage, isLanguageSupported, isGrammarLoaded, getSupportedLanguages, initGrammars, loadGrammarsForLanguages, loadAllGrammars } from './grammars';
