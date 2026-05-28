import * as fs from 'fs';
import * as fsp from 'fs/promises';
import type { ResultStore } from './interfaces';
import type { CodeGraphConfig, ExtractionResult } from '../types';
import { detectLanguage, isLanguageSupported } from './grammars';
import { extractFromSource } from './tree-sitter';
import { validatePathWithinRoot } from '../utils';
import { logWarn } from '../errors';
import * as fsSync from 'fs';
import type { ResolutionContext } from '../resolution/types';
import { detectFrameworks } from '../resolution/frameworks';

export class SingleFileIndexer {
  private detectedFrameworkNames: string[] | null = null;

  constructor(
    private rootDir: string,
    private config: CodeGraphConfig,
    private resultStore: ResultStore,
  ) {}

  setFrameworkNames(names: string[]): void {
    this.detectedFrameworkNames = names;
  }

  private buildDetectionContext(files?: string[]): ResolutionContext {
    const rootDir = this.rootDir;
    return {
      getNodesInFile: () => [],
      getNodesByName: () => [],
      getNodesByQualifiedName: () => [],
      getNodesByKind: () => [],
      getNodesByLowerName: () => [],
      getImportMappings: () => [],
      getProjectAliases: () => null,
      getAllFiles: () => files ?? [],
      getProjectRoot: () => rootDir,
      fileExists: (filePath: string) => {
        const full = validatePathWithinRoot(rootDir, filePath);
        if (!full) return false;
        try { return fsSync.existsSync(full); } catch { return false; }
      },
      readFile: (filePath: string) => {
        const full = validatePathWithinRoot(rootDir, filePath);
        if (!full) return null;
        try { return fsSync.readFileSync(full, 'utf-8'); } catch { return null; }
      },
    };
  }

  private ensureDetectedFrameworks(files?: string[]): string[] {
    if (this.detectedFrameworkNames === null) {
      const context = this.buildDetectionContext(files);
      this.detectedFrameworkNames = detectFrameworks(context).map((r) => r.name);
    }
    return this.detectedFrameworkNames;
  }

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

  async indexFileWithContent(
    relativePath: string,
    content: string,
    stats: fs.Stats
  ): Promise<ExtractionResult> {
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

    const frameworkNames = this.ensureDetectedFrameworks();
    const result = extractFromSource(relativePath, content, language, frameworkNames);

    if (result.nodes.length > 0 || result.errors.length === 0) {
      this.resultStore.storeExtractionResult(relativePath, content, language, stats, result);
    }

    return result;
  }
}
