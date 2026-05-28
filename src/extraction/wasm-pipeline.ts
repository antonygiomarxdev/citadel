import * as fs from 'fs';
import * as fsp from 'fs/promises';
import type { ParsePipeline, PipelineResult, IndexProgress, ResultStore } from './interfaces';
import type { ExtractionResult, CodeGraphConfig } from '../types';
import { ParseWorkerPool } from './parse-worker-pool';
import { detectLanguage } from './grammars';
import { validatePathWithinRoot } from '../utils';
import { logWarn } from '../errors';

const FILE_IO_BATCH_SIZE = 200;

export class WasmPipeline implements ParsePipeline {
  constructor(
    private rootDir: string,
    private resultStore: ResultStore,
    private config: CodeGraphConfig,
  ) {}

  async execute(
    files: string[],
    _rootDir: string,
    frameworkNames: string[],
    onProgress?: (progress: IndexProgress) => void,
    signal?: AbortSignal
  ): Promise<PipelineResult> {
    if (files.length === 0) {
      return { nodesCreated: 0, edgesCreated: 0, filesIndexed: 0, filesSkipped: 0, filesErrored: 0, errors: [] };
    }

    const neededLanguages = [...new Set(files.map((f) => detectLanguage(f)))];
    if (neededLanguages.includes('c') && !neededLanguages.includes('cpp')) {
      neededLanguages.push('cpp');
    }
    const pool = new ParseWorkerPool(neededLanguages, frameworkNames);
    await pool.initialize();

    let totalNodes = 0;
    let totalEdges = 0;
    let filesIndexed = 0;
    let filesSkipped = 0;
    let filesErrored = 0;
    const errors: PipelineResult['errors'] = [];
    let processed = 0;
    const total = files.length;

    const tallyResult = (result: ExtractionResult): void => {
      if (result.errors.length > 0) {
        for (const err of result.errors) {
          if (!err.filePath) err.filePath = result.nodes[0]?.filePath;
        }
        errors.push(...result.errors.map(e => ({
          message: e.message,
          filePath: e.filePath,
          severity: e.severity,
          code: e.code,
        })));
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
          pool.terminate();
          return { nodesCreated: totalNodes, edgesCreated: totalEdges, filesIndexed, filesSkipped, filesErrored, errors };
        }

        const batch = files.slice(i, i + FILE_IO_BATCH_SIZE);

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

        for (const { filePath, content, stats, error } of fileContents) {
          if (signal?.aborted) {
            pool.terminate();
            return { nodesCreated: totalNodes, edgesCreated: totalEdges, filesIndexed, filesSkipped, filesErrored, errors };
          }

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

          if (result.nodes.length > 0 || result.errors.length === 0) {
            const language = detectLanguage(filePath, content);
            this.resultStore.storeExtractionResult(filePath, content, language, stats, result);
          }

          tallyResult(result);
          onProgress?.({ phase: 'parsing', current: processed, total });
        }
      }

    // Retry pass: files that failed due to WASM memory corruption may succeed
    // on a fresh worker with a clean heap. Recycle before each attempt so
    // every file gets the absolute cleanest WASM state possible.
    const retryableErrors = errors.filter(
      (e) => e.code === 'parse_error' && e.filePath &&
        (e.message.includes('Worker exited') || e.message.includes('memory access out of bounds'))
    );

    if (retryableErrors.length > 0 && pool.hasWorker) {
      const stillFailing: typeof retryableErrors = [];

      for (const errEntry of retryableErrors) {
        const filePath = errEntry.filePath!;
        if (signal?.aborted) break;

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
            this.resultStore.storeExtractionResult(filePath, content, language, stats, result);
          }

          const idx = errors.indexOf(errEntry);
          if (idx >= 0) errors.splice(idx, 1);
          filesErrored--;
          filesIndexed++;
          totalNodes += result.nodes.length;
          totalEdges += result.edges.length;
        }
      }

      // Last resort: for files that still crash on a clean worker, strip
      // comment-only lines to reduce WASM memory pressure. Many compiler
      // test files are 90%+ comments (CHECK directives) that don't contribute
      // code nodes but consume parser memory.
      if (stillFailing.length > 0) {
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
              this.resultStore.storeExtractionResult(filePath, fullContent, language, stats, result);
            }

            const idx = errors.indexOf(errEntry);
            if (idx >= 0) errors.splice(idx, 1);
            filesErrored--;
            filesIndexed++;
            totalNodes += result.nodes.length;
            totalEdges += result.edges.length;
          }
        }
      }
    }

    pool.terminate();

    return {
      nodesCreated: totalNodes,
      edgesCreated: totalEdges,
      filesIndexed,
      filesSkipped,
      filesErrored,
      errors,
    };
  }
}
