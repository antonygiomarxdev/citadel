import type { CodeGraphConfig } from '../types';
import type { QueryBuilder } from '../db/queries';
import { detectFileChanges } from './change-detector';
import { SingleFileIndexer } from './single-file-indexer';
import { initGrammars, detectLanguage, loadGrammarsForLanguages } from './grammars';
import type { IndexProgress } from './interfaces';

export interface SyncResult {
  filesChecked: number;
  filesAdded: number;
  filesModified: number;
  filesRemoved: number;
  nodesUpdated: number;
  durationMs: number;
  changedFilePaths?: string[];
}

export class SyncService {
  constructor(
    private rootDir: string,
    private config: CodeGraphConfig,
    private queries: QueryBuilder,
    private fileIndexer: SingleFileIndexer,
  ) {}

  getChangedFiles(): { added: string[]; modified: string[]; removed: string[] } {
    return detectFileChanges(this.rootDir, this.config, this.queries);
  }

  async sync(onProgress?: (progress: IndexProgress) => void): Promise<SyncResult> {
    await initGrammars();
    const startTime = Date.now();
    const changedFilePaths: string[] = [];
    let nodesUpdated = 0;

    onProgress?.({ phase: 'scanning', current: 0, total: 0 });

    const changes = detectFileChanges(this.rootDir, this.config, this.queries);

    for (const filePath of changes.removed) {
      this.queries.deleteFile(filePath);
    }

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
      const result = await this.fileIndexer.indexFile(filePath);
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
}
