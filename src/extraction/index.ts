/**
 * Extraction Orchestrator
 *
 * Coordinates file scanning, parsing, and database storage.
 */

import * as fs from 'fs';
import * as fsp from 'fs/promises';
// Native Rust extraction: tree-sitter + rayon parallel parsing.
// Replace WASM extraction for TS/JS/Python files when the native backend is available.
let nativeExtractFiles: ((files: string[][], frameworkNames: string[]) => any[]) | null = null;
function getNativeExtractor() {
  if (nativeExtractFiles !== null) return nativeExtractFiles;
  try {
    const napi = require('../citadel-native.linux-x64-gnu.node');
    nativeExtractFiles = (files: string[][], fw: string[]) => napi.extractFiles(files, fw);
    return nativeExtractFiles;
  } catch {
    nativeExtractFiles = undefined as any;
    return null;
  }
}
// Languages that have native Rust tree-sitter extractors.
const NATIVE_EXTRACTOR_LANGS = new Set(['typescript', 'javascript', 'tsx', 'jsx', 'python', 'go', 'rust', 'java']);
import {
  Language,
  FileRecord,
  ExtractionResult,
  ExtractionError,
  CodeGraphConfig,
} from '../types';
import { QueryBuilder } from '../db/queries';
import { extractFromSource } from './tree-sitter';
import { detectLanguage, isLanguageSupported, initGrammars, loadGrammarsForLanguages } from './grammars';
import { logWarn } from '../errors';
import { validatePathWithinRoot } from '../utils';
import { hashContent } from '../utils/hash';
import {
  scanDirectory,
  scanDirectoryAsync,
  shouldIncludeFile,
  getGitVisibleFiles,
  getGitChangedFiles,
  GitChanges,
} from './file-scanner';
import { detectFrameworks } from '../resolution/frameworks';
import type { ResolutionContext } from '../resolution/types';
import { ParseWorkerPool } from './parse-worker-pool';
import { detectFileChanges } from './change-detector';

// Re-export for external consumers
export { scanDirectory, scanDirectoryAsync, shouldIncludeFile, getGitVisibleFiles, getGitChangedFiles };
export type { GitChanges };


const FILE_IO_BATCH_SIZE = 10;

/**
 * Progress callback for indexing operations
 */
export interface IndexProgress {
  phase: 'scanning' | 'parsing' | 'storing' | 'resolving';
  current: number;
  total: number;
  currentFile?: string;
}

/**
 * Result of an indexing operation
 */
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

/**
 * Result of a sync operation
 */
export interface SyncResult {
  filesChecked: number;
  filesAdded: number;
  filesModified: number;
  filesRemoved: number;
  nodesUpdated: number;
  durationMs: number;
  changedFilePaths?: string[];
}

/**
 * Extraction orchestrator
 */
export class ExtractionOrchestrator {
  private rootDir: string;
  private config: CodeGraphConfig;
  private queries: QueryBuilder;
  /**
   * Names of frameworks detected for this project, populated by indexAll().
   * Passed to extractFromSource so framework-specific extractors (route nodes,
   * middleware, etc.) run after the tree-sitter pass. Cleared if detection
   * hasn't run yet so single-file re-index paths can detect on the spot.
   */
  private detectedFrameworkNames: string[] | null = null;

  constructor(rootDir: string, config: CodeGraphConfig, queries: QueryBuilder) {
    this.rootDir = rootDir;
    this.config = config;
    this.queries = queries;
  }

  /**
   * Build a filesystem-backed ResolutionContext sufficient for framework
   * detection. Graph-query methods (getNodesByName etc.) return empty because
   * the DB hasn't been populated yet, but detect() only uses readFile,
   * fileExists, and getAllFiles, so that's fine.
   */
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
        try {
          return fs.existsSync(full);
        } catch {
          return false;
        }
      },
      readFile: (relativePath: string) => {
        const full = validatePathWithinRoot(rootDir, relativePath);
        if (!full) return null;
        try {
          return fs.readFileSync(full, 'utf-8');
        } catch {
          return null;
        }
      },
    };
  }

  /**
   * Detect frameworks on demand using the current scanned files (or a fresh
   * scan if none are provided). Cached on the orchestrator so repeat calls
   * inside a single run don't re-scan.
   */
  private ensureDetectedFrameworks(files?: string[]): string[] {
    if (this.detectedFrameworkNames !== null) return this.detectedFrameworkNames;
    const fileList = files ?? scanDirectory(this.rootDir, this.config);
    const context = this.buildDetectionContext(fileList);
    this.detectedFrameworkNames = detectFrameworks(context).map((r) => r.name);
    return this.detectedFrameworkNames;
  }

  /**
   * Index all files in the project
   */
  async indexAll(
    onProgress?: (progress: IndexProgress) => void,
    signal?: AbortSignal,
    verbose?: boolean
  ): Promise<IndexResult> {
    await initGrammars();
    const startTime = Date.now();
    const errors: ExtractionError[] = [];
    let filesIndexed = 0;
    let filesSkipped = 0;
    let filesErrored = 0;
    let totalNodes = 0;
    let totalEdges = 0;

    const log = verbose
      ? (msg: string) => { console.log(`[worker] ${msg}`); }
      : (_msg: string) => {};

    // Phase 1: Scan for files
    onProgress?.({
      phase: 'scanning',
      current: 0,
      total: 0,
    });

    const files = await scanDirectoryAsync(this.rootDir, this.config, (current, file) => {
      onProgress?.({
        phase: 'scanning',
        current,
        total: 0,
        currentFile: file,
      });
    });

    // Detect frameworks once per indexAll run using the scanned file list.
    // Names are passed to each parse call so framework-specific extractors
    // (route nodes, middleware, etc.) run after the tree-sitter pass.
    // Framework detection is reset each run so adding e.g. requirements.txt
    // between runs is picked up without restarting the process.
    this.detectedFrameworkNames = null;
    const frameworkNames = this.ensureDetectedFrameworks(files);

    if (signal?.aborted) {
      return {
        success: false,
        filesIndexed: 0,
        filesSkipped: 0,
        filesErrored: 0,
        nodesCreated: 0,
        edgesCreated: 0,
        errors: [{ message: 'Aborted', severity: 'error' }],
        durationMs: Date.now() - startTime,
      };
    }

    // Phase 2: Parse files in a worker thread (keeps main thread unblocked for UI)
    const total = files.length;
    let processed = 0;

    // Emit parsing phase immediately so the progress bar appears during worker setup.
    // The yield lets the shimmer worker flush the phase transition to stdout before
    // the main thread starts synchronous grammar detection work.
    onProgress?.({
      phase: 'parsing',
      current: 0,
      total,
    });
    await new Promise(resolve => setImmediate(resolve));
    // Create the parse worker pool (handles worker lifecycle, fallback, timeouts).
    // The pool auto-detects whether a compiled worker is available and falls back
    // to in-process parsing in test environments.
    const neededLanguages = [...new Set(files.map((f) => detectLanguage(f)))];
    if (neededLanguages.includes('c') && !neededLanguages.includes('cpp')) {
      neededLanguages.push('cpp');
    }
    const pool = new ParseWorkerPool(neededLanguages, frameworkNames, verbose);
    await pool.initialize();

    const tallyResult = (result: ExtractionResult): void => {
      if (result.errors.length > 0) {
        for (const err of result.errors) {
          if (!err.filePath) err.filePath = result.nodes[0]?.filePath;
        }
        errors.push(...result.errors);
      }
      if (result.nodes.length > 0) {
        filesIndexed++;
        totalNodes += result.nodes.length;
        totalEdges += result.edges.length;
      } else if (result.errors.some((e) => e.severity === 'error')) {
        filesErrored++;
      } else {
        filesSkipped++;
      }
    };

    for (let i = 0; i < files.length; i += FILE_IO_BATCH_SIZE) {
      if (signal?.aborted) {
        if (pool.hasWorker) pool.terminate();
        return {
          success: false,
          filesIndexed,
          filesSkipped,
          filesErrored,
          nodesCreated: totalNodes,
          edgesCreated: totalEdges,
          errors: [{ message: 'Aborted', severity: 'error' }, ...errors],
          durationMs: Date.now() - startTime,
        };
      }

      const batch = files.slice(i, i + FILE_IO_BATCH_SIZE);

      // Read files in parallel (with path validation before any I/O)
      let fileContents = await Promise.all(
        batch.map(async (fp) => {
          try {
            const fullPath = validatePathWithinRoot(this.rootDir, fp);
            if (!fullPath) {
              logWarn('Path traversal blocked in batch reader', { filePath: fp });
              return { filePath: fp, content: null as string | null, stats: null as fs.Stats | null, error: new Error('Path traversal blocked') };
            }
            const content = await fsp.readFile(fullPath, 'utf-8');
            const stats = await fsp.stat(fullPath);
            return { filePath: fp, content, stats, error: null as Error | null };
          } catch (err) {
            return { filePath: fp, content: null as string | null, stats: null as fs.Stats | null, error: err as Error };
          }
        })
      );

      // Fast path: use native Rust extraction for TS/JS/Python files.
      // tree-sitter + rayon par_iter() is ~5x faster than WASM per file
      // and scales linearly across CPU cores.
      const nativeExtract = getNativeExtractor();
      if (nativeExtract) {
        const nativeInputs: string[][] = [];
        const nativeIndices: number[] = [];
        const nativeLangMap: string[] = [];
        const nativeStats: (fs.Stats | null)[] = [];
        for (let j = 0; j < fileContents.length; j++) {
          const fc = fileContents[j];
          if (!fc || fc.error || fc.content === null) continue;
          const lang = detectLanguage(fc.filePath, fc.content!);
          if (NATIVE_EXTRACTOR_LANGS.has(lang)) {
            nativeInputs.push([fc.filePath, fc.content]);
            nativeIndices.push(j);
            nativeLangMap.push(lang);
            nativeStats.push(fc.stats);
          }
        }
        if (nativeInputs.length > 0) {
          let nativeResults: any[];
          try {
            nativeResults = nativeExtract(nativeInputs, frameworkNames);
          } catch (err) {
            logWarn('Native extraction failed, falling back to WASM', { error: String(err) });
            nativeResults = [];
          }
          if (nativeResults.length === 0) {
            // Native extraction returned empty — fall through to WASM.
          } else {
          for (let r = 0; r < nativeResults.length; r++) {
            const idx = nativeIndices[r]!;
            const fc = fileContents[idx];
            if (!fc || fc.content === null) continue;
            const lang = nativeLangMap[r];
            const result = nativeResults[r] as ExtractionResult;
            processed++;
            if (result.nodes.length > 0 || result.errors.length === 0) {
              this.storeExtractionResult(fc.filePath, fc.content!, lang as any, fc.stats!, result);
            }
            this.storeExtractionResult(fc.filePath, fc.content!, lang as Language, fc.stats!, result);
            tallyResult(result);
            onProgress?.({ phase: 'parsing', current: processed, total });
          }
          // Skip these files in the WASM loop below
          const skipSet = new Set(nativeIndices);
          fileContents = fileContents.filter((_, j) => !skipSet.has(j));
          } // end else nativeResults
        }
      }

      // Send to worker for parsing, store results on main thread
      for (const { filePath, content, stats, error } of fileContents) {
        if (signal?.aborted) {
          if (pool.hasWorker) pool.terminate();
          return {
            success: false,
            filesIndexed,
            filesSkipped,
            filesErrored,
            nodesCreated: totalNodes,
            edgesCreated: totalEdges,
            errors: [{ message: 'Aborted', severity: 'error' }, ...errors],
            durationMs: Date.now() - startTime,
          };
        }

        // Report progress before parsing (show current file being worked on)
        onProgress?.({
          phase: 'parsing',
          current: processed,
          total,
          currentFile: filePath,
        });

        if (error || content === null || stats === null) {
          processed++;
          filesErrored++;
          errors.push({
            message: `Failed to read file: ${error instanceof Error ? error.message : String(error)}`,
            filePath,
            severity: 'error',
            code: 'read_error',
          });
          continue;
        }

        // Honour config.maxFileSize. Without this check, vendored
        // generated headers, minified bundles, and other multi-MB
        // files get indexed despite the user setting a size cap —
        // wasting WASM heap and the worker recycle budget on inputs
        // the user explicitly opted out of. The single-file extractFile
        // path already enforces this; the bulk path used to silently
        // skip the check.
        if (stats.size > this.config.maxFileSize) {
          processed++;
          filesSkipped++;
          errors.push({
            message: `File exceeds max size (${stats.size} > ${this.config.maxFileSize})`,
            filePath,
            severity: 'warning',
            code: 'size_exceeded',
          });
          onProgress?.({ phase: 'parsing', current: processed, total });
          continue;
        }

        // Parse in worker thread (main thread stays unblocked).
        // Wrapped in try/catch to handle worker timeouts and crashes gracefully.
        let result: ExtractionResult;
        try {
          result = await pool.requestParse(filePath, content);
        } catch (parseErr) {
          processed++;
          filesErrored++;
          errors.push({
            message: parseErr instanceof Error ? parseErr.message : String(parseErr),
            filePath,
            severity: 'error',
            code: 'parse_error',
          });
          continue;
        }

        processed++;

        // Store in database on main thread (SQLite is not thread-safe)
        if (result.nodes.length > 0 || result.errors.length === 0) {
          const language = detectLanguage(filePath, content);
          this.storeExtractionResult(filePath, content, language, stats, result);
        }

        tallyResult(result);
        onProgress?.({ phase: 'parsing', current: processed, total });
      }
    }

    // Report 100% so the progress bar doesn't hang at 99%
    onProgress?.({
      phase: 'parsing',
      current: total,
      total,
    });

    // Yield so the shimmer worker's buffered stdout writes can flush.
    // Worker thread stdout is proxied through the main thread's event loop,
    // so synchronous work here blocks the animation from rendering.
    await new Promise(resolve => setImmediate(resolve));

    // Retry pass: files that failed due to WASM memory corruption may succeed
    // on a fresh worker with a clean heap. Recycle before each attempt so
    // every file gets the absolute cleanest WASM state possible.
    const retryableErrors = errors.filter(
      (e) => e.code === 'parse_error' && e.filePath &&
        (e.message.includes('Worker exited') || e.message.includes('memory access out of bounds'))
    );

    if (retryableErrors.length > 0 && pool.hasWorker) {
      log(`Retrying ${retryableErrors.length} files that failed due to WASM memory errors...`);

      const stillFailing: typeof retryableErrors = [];

      for (const errEntry of retryableErrors) {
        const filePath = errEntry.filePath!;
        if (signal?.aborted) break;

        // Fresh worker for every retry — maximum WASM headroom
        // Fresh worker for every retry — maximum WASM headroom
        pool.recycleWorker();

        let content: string;
        try {
          const fullPath = validatePathWithinRoot(this.rootDir, filePath);
          if (!fullPath) continue;
          content = await fsp.readFile(fullPath, 'utf-8');
        } catch {
          continue;
        }

        let result: ExtractionResult;
        try {
          result = await pool.requestParse(filePath, content);
        } catch {
          stillFailing.push(errEntry);
          continue;
        }

        if (result.nodes.length > 0 || result.errors.length === 0) {
          const language = detectLanguage(filePath, content);
          const fullPath = validatePathWithinRoot(this.rootDir, filePath);
          const stats = fullPath ? await fsp.stat(fullPath) : null;
          if (stats) {
            this.storeExtractionResult(filePath, content, language, stats, result);
          }

          const idx = errors.indexOf(errEntry);
          if (idx >= 0) errors.splice(idx, 1);
          filesErrored--;
          filesIndexed++;
          totalNodes += result.nodes.length;
          totalEdges += result.edges.length;
          log(`Retry OK: ${filePath} (${result.nodes.length} nodes)`);
        }
      }

      // Last resort: for files that still crash on a clean worker, strip
      // comment-only lines to reduce WASM memory pressure. Many compiler
      // test files are 90%+ comments (CHECK directives) that don't contribute
      // code nodes but consume parser memory.
      if (stillFailing.length > 0) {
        log(`${stillFailing.length} files still failing — retrying with comments stripped...`);

        for (const errEntry of stillFailing) {
          const filePath = errEntry.filePath!;
          if (signal?.aborted) break;

          // Fresh worker for every retry — maximum WASM headroom
        pool.recycleWorker();

          let fullContent: string;
          try {
            const fullPath = validatePathWithinRoot(this.rootDir, filePath);
            if (!fullPath) continue;
            fullContent = await fsp.readFile(fullPath, 'utf-8');
          } catch {
            continue;
          }

          // Strip lines that are entirely comments (preserving line numbers
          // by replacing with empty lines so node positions stay correct)
          const stripped = fullContent
            .split('\n')
            .map(line => /^\s*\/\//.test(line) ? '' : line)
            .join('\n');

          let result: ExtractionResult;
          try {
            result = await pool.requestParse(filePath, stripped);
          } catch {
            continue;
          }

          if (result.nodes.length > 0 || result.errors.length === 0) {
            const language = detectLanguage(filePath, fullContent);
            const fullPath = validatePathWithinRoot(this.rootDir, filePath);
            const stats = fullPath ? await fsp.stat(fullPath) : null;
            if (stats) {
              this.storeExtractionResult(filePath, fullContent, language, stats, result);
            }

            const idx = errors.indexOf(errEntry);
            if (idx >= 0) errors.splice(idx, 1);
            filesErrored--;
            filesIndexed++;
            totalNodes += result.nodes.length;
            totalEdges += result.edges.length;
            log(`Retry (stripped) OK: ${filePath} (${result.nodes.length} nodes)`);
          }
        }
      }
    }

    // Shut down parse worker
    pool.terminate();

    return {
      success: filesIndexed > 0 || errors.filter((e) => e.severity === 'error').length === 0,
      filesIndexed,
      filesSkipped,
      filesErrored,
      nodesCreated: totalNodes,
      edgesCreated: totalEdges,
      errors,
      durationMs: Date.now() - startTime,
    };
  }

  /**
   * Index specific files
   */
  async indexFiles(filePaths: string[]): Promise<IndexResult> {
    const startTime = Date.now();
    const errors: ExtractionError[] = [];
    let filesIndexed = 0;
    let filesSkipped = 0;
    let filesErrored = 0;
    let totalNodes = 0;
    let totalEdges = 0;

    for (const filePath of filePaths) {
      const result = await this.indexFile(filePath);

      if (result.errors.length > 0) {
        errors.push(...result.errors);
      }

      if (result.nodes.length > 0) {
        filesIndexed++;
        totalNodes += result.nodes.length;
        totalEdges += result.edges.length;
      } else if (result.errors.some((e) => e.severity === 'error')) {
        filesErrored++;
      } else {
        filesSkipped++;
      }
    }

    return {
      success: filesIndexed > 0 || errors.filter((e) => e.severity === 'error').length === 0,
      filesIndexed,
      filesSkipped,
      filesErrored,
      nodesCreated: totalNodes,
      edgesCreated: totalEdges,
      errors,
      durationMs: Date.now() - startTime,
    };
  }

  /**
   * Index a single file
   */
  async indexFile(relativePath: string): Promise<ExtractionResult> {
    const fullPath = validatePathWithinRoot(this.rootDir, relativePath);

    if (!fullPath) {
      return {
        nodes: [],
        edges: [],
        unresolvedReferences: [],
        errors: [{ message: `Path traversal blocked: ${relativePath}`, filePath: relativePath, severity: 'error', code: 'path_traversal' }],
        durationMs: 0,
      };
    }

    // Read file content and stats
    let content: string;
    let stats: fs.Stats;
    try {
      stats = await fsp.stat(fullPath);
      content = await fsp.readFile(fullPath, 'utf-8');
    } catch (error) {
      return {
        nodes: [],
        edges: [],
        unresolvedReferences: [],
        errors: [
          {
            message: `Failed to read file: ${error instanceof Error ? error.message : String(error)}`,
            filePath: relativePath,
            severity: 'error',
            code: 'read_error',
          },
        ],
        durationMs: 0,
      };
    }

    return this.indexFileWithContent(relativePath, content, stats);
  }

  /**
   * Index a single file with pre-read content and stats.
   * Used by the parallel batch reader to avoid redundant file I/O.
   */
  async indexFileWithContent(
    relativePath: string,
    content: string,
    stats: fs.Stats
  ): Promise<ExtractionResult> {
    // Prevent path traversal
    const fullPath = validatePathWithinRoot(this.rootDir, relativePath);
    if (!fullPath) {
      logWarn('Path traversal blocked in indexFileWithContent', { relativePath });
      return {
        nodes: [],
        edges: [],
        unresolvedReferences: [],
        errors: [{ message: 'Path traversal blocked', filePath: relativePath, severity: 'error', code: 'path_traversal' }],
        durationMs: 0,
      };
    }

    // Check file size
    if (stats.size > this.config.maxFileSize) {
      return {
        nodes: [],
        edges: [],
        unresolvedReferences: [],
        errors: [
          {
            message: `File exceeds max size (${stats.size} > ${this.config.maxFileSize})`,
            filePath: relativePath,
            severity: 'warning',
            code: 'size_exceeded',
          },
        ],
        durationMs: 0,
      };
    }

    // Detect language
    const language = detectLanguage(relativePath, content);
    if (!isLanguageSupported(language)) {
      return {
        nodes: [],
        edges: [],
        unresolvedReferences: [],
        errors: [],
        durationMs: 0,
      };
    }

    // Extract from source. Use cached framework names if indexAll has run,
    // otherwise detect on the spot so single-file re-index paths still emit
    // route nodes / middleware / etc.
    const frameworkNames = this.ensureDetectedFrameworks();
    const result = extractFromSource(relativePath, content, language, frameworkNames);

    // Store in database
    if (result.nodes.length > 0 || result.errors.length === 0) {
      this.storeExtractionResult(relativePath, content, language, stats, result);
    }

    return result;
  }

  /**
   * Store extraction result in database
   */
  private storeExtractionResult(
    filePath: string,
    content: string,
    language: Language,
    stats: fs.Stats,
    result: ExtractionResult
  ): void {
    const contentHash = hashContent(content);

    // Check if file already exists and hasn't changed
    const existingFile = this.queries.getFileByPath(filePath);
    if (existingFile && existingFile.contentHash === contentHash) {
      return; // No changes
    }

    // Delete existing data for this file
    if (existingFile) {
      this.queries.deleteFile(filePath);
    }

    // Filter out nodes with missing required fields before insertion.
    // This prevents FK violations when edges reference nodes that would
    // be silently skipped by insertNode() (see issue #42).
    const validNodes = result.nodes.filter((n) => n.id && n.kind && n.name && n.filePath && n.language);

    // Insert nodes
    if (validNodes.length > 0) {
      this.queries.insertNodes(validNodes);
    }

    // Filter edges to only reference nodes that were actually inserted
    if (result.edges.length > 0) {
      const insertedIds = new Set(validNodes.map((n) => n.id));
      const validEdges = result.edges.filter(
        (e) => insertedIds.has(e.source) && insertedIds.has(e.target)
      );
      if (validEdges.length > 0) {
        this.queries.insertEdges(validEdges);
      }
    }

    // Insert unresolved references in batch with denormalized filePath/language
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

    // Insert file record
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

  /**
   * Sync with current file state.
   * Uses git status as a fast path when available, falling back to full scan.
   */
  async sync(onProgress?: (progress: IndexProgress) => void): Promise<SyncResult> {
    await initGrammars();
    const startTime = Date.now();
    const changedFilePaths: string[] = [];
    let nodesUpdated = 0;

    onProgress?.({ phase: 'scanning', current: 0, total: 0 });

    const changes = detectFileChanges(this.rootDir, this.config, this.queries);

    // Handle removed files
    for (const filePath of changes.removed) {
      this.queries.deleteFile(filePath);
    }

    // Collect files to index (added + modified)
    const filesToIndex = [...changes.added, ...changes.modified];
    changedFilePaths.push(...filesToIndex);

    const filesChecked = changes.added.length + changes.modified.length + changes.removed.length;
    const filesAdded = changes.added.length;
    const filesModified = changes.modified.length;
    const filesRemoved = changes.removed.length;

    if (filesToIndex.length > 0) {
      const neededLanguages = [...new Set(filesToIndex.map((f) => detectLanguage(f)))];
      if (neededLanguages.includes('c') && !neededLanguages.includes('cpp')) {
        neededLanguages.push('cpp');
      }
      await loadGrammarsForLanguages(neededLanguages);
    }

    const total = filesToIndex.length;
    for (let i = 0; i < filesToIndex.length; i++) {
      const filePath = filesToIndex[i]!;
      onProgress?.({ phase: 'parsing', current: i + 1, total, currentFile: filePath });
      const result = await this.indexFile(filePath);
      nodesUpdated += result.nodes.length;
    }

    return {
      filesChecked,
      filesAdded,
      filesModified,
      filesRemoved,
      nodesUpdated,
      durationMs: Date.now() - startTime,
      changedFilePaths: changedFilePaths.length > 0 ? changedFilePaths : undefined,
    };
  }

  getChangedFiles(): { added: string[]; modified: string[]; removed: string[] } {
    return detectFileChanges(this.rootDir, this.config, this.queries);
  }
}

// Re-export useful types and functions
export { extractFromSource } from './tree-sitter';
export { detectLanguage, isLanguageSupported, isGrammarLoaded, getSupportedLanguages, initGrammars, loadGrammarsForLanguages, loadAllGrammars } from './grammars';
