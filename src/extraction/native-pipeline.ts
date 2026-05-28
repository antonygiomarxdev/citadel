import * as os from 'os';
import type { ParsePipeline, PipelineResult, IndexProgress } from './interfaces';

interface JsIndexResult {
  filesIndexed: number;
  filesErrored: number;
  filesSkipped: number;
  nodesCreated: number;
  edgesCreated: number;
  errors: string[];
}

interface NapiDatabase {
  open(path: string): void;
  indexFiles(paths: string[], rootDir: string, fw: string[], nw: number): JsIndexResult;
  close(): void;
}

let napiModule: { Database: new () => NapiDatabase } | null = null;
(function () {
  for (const tryPath of [
    '../citadel-native.linux-x64-gnu.node',            // dist/extraction/ -> dist/
    __dirname + '/../citadel-native.linux-x64-gnu.node', // absolute: when __dirname = src/extraction/
  ]) {
    try { napiModule = require(tryPath); if (napiModule) return; } catch {}
  }
})();

export class NativePipeline implements ParsePipeline {
  readonly available: boolean;

  constructor(
    private rootDir: string,
    private dbPath: string,
    private frameworkNames: string[],
  ) {
    this.available = napiModule !== null;
  }

  async execute(
    files: string[],
    _rootDir: string,
    _frameworkNames: string[],
    onProgress?: (progress: IndexProgress) => void,
    _signal?: AbortSignal,
  ): Promise<PipelineResult> {
    if (files.length === 0 || !napiModule) {
      return { nodesCreated: 0, edgesCreated: 0, filesIndexed: 0, filesSkipped: 0, filesErrored: 0, errors: [] };
    }

    const numWorkers = Math.max(1, (os.cpus?.().length ?? 1) - 1);
    const db = new napiModule.Database();

    try {
      db.open(this.dbPath);
      const result = db.indexFiles(files, this.rootDir, this.frameworkNames, numWorkers);

      onProgress?.({ phase: 'parsing', current: files.length, total: files.length });

      return {
        nodesCreated: result.nodesCreated,
        edgesCreated: result.edgesCreated,
        filesIndexed: result.filesIndexed,
        filesSkipped: result.filesSkipped,
        filesErrored: result.filesErrored,
        errors: result.errors.map((msg) => ({ message: msg, severity: 'error' as const })),
      };
    } finally {
      db.close();
    }
  }
}
