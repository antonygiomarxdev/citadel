/**
 * Parse Worker Pool
 *
 * Manages a dedicated worker thread for tree-sitter parsing. Keeps the
 * main thread free for UI operations while handling WASM memory limits
 * through periodic worker recycling (V8 isolate restart).
 *
 * Falls back to in-process parsing when no compiled worker is available
 * (test environments).
 */

import * as path from 'path';
import * as fs from 'fs';
import type { Worker } from 'worker_threads';
import type { ExtractionResult, Language } from '../types';
import { extractFromSource } from './tree-sitter';
import { detectLanguage, loadGrammarsForLanguages } from './grammars';
import { logWarn } from '../errors';

const PARSE_TIMEOUT_MS = 10_000;
const WORKER_RECYCLE_INTERVAL = 250;

interface PendingParse {
  resolve: (result: ExtractionResult) => void;
  reject: (err: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

export class ParseWorkerPool {
  private worker: Worker | null = null;
  private workerPath: string;
  private WorkerClass: typeof Worker | null = null;
  private nextId = 0;
  private parseCount = 0;
  private pending = new Map<number, PendingParse>();
  private neededLanguages: string[];
  private frameworkNames: string[];
  private log: (msg: string) => void;

  constructor(
    neededLanguages: string[],
    frameworkNames: string[],
    verbose?: boolean
  ) {
    this.neededLanguages = neededLanguages;
    this.frameworkNames = frameworkNames;
    this.log = verbose
      ? (msg: string) => { console.log(`[worker] ${msg}`); }
      : (_msg: string) => {};

    this.workerPath = path.join(__dirname, 'parse-worker.js');
  }

  async initialize(): Promise<void> {
    if (fs.existsSync(this.workerPath)) {
      const { Worker } = await import('worker_threads');
      this.WorkerClass = Worker;
      await this.ensureWorker();
    } else {
      await loadGrammarsForLanguages(this.neededLanguages as Language[]);
    }
  }

  private ensureWorker(): Promise<Worker> {
    if (this.worker) return Promise.resolve(this.worker);
    this.log('Spawning new parse worker...');
    this.worker = new this.WorkerClass!(this.workerPath);
    this.attachHandlers();

    return new Promise<void>((resolve, reject) => {
      this.worker!.once('message', (msg: { type: string }) => {
        if (msg.type === 'grammars-loaded') resolve();
        else reject(new Error(`Unexpected message: ${msg.type}`));
      });
      this.worker!.postMessage({ type: 'load-grammars', languages: this.neededLanguages });
    }).then(() => this.worker!);
  }

  private attachHandlers(): void {
    const w = this.worker!;

    w.on('message', (msg: { type: string; id?: number; result?: ExtractionResult }) => {
      if (msg.type === 'parse-result' && msg.id !== undefined) {
        const pending = this.pending.get(msg.id);
        if (pending) {
          clearTimeout(pending.timer);
          this.pending.delete(msg.id);
          pending.resolve(msg.result!);
        }
      }
    });

    w.on('error', (err) => {
      logWarn('Parse worker error', { error: err.message });
      this.rejectAllPending(`Worker error: ${err.message}`);
    });

    w.on('exit', (code) => {
      if (code !== 0 && this.pending.size > 0) {
        logWarn('Parse worker exited unexpectedly', { code });
        this.rejectAllPending(`Worker exited with code ${code}`);
      }
      if (this.worker === w) {
        this.worker = null;
        this.parseCount = 0;
      }
    });
  }

  private rejectAllPending(reason: string): void {
    for (const [id, pending] of this.pending) {
      clearTimeout(pending.timer);
      this.pending.delete(id);
      pending.reject(new Error(reason));
    }
  }

  recycleWorker(): void {
    if (!this.worker) return;
    this.log(`Recycling worker after ${this.parseCount} parses (heap: ${Math.round(process.memoryUsage().rss / 1024 / 1024)}MB RSS)`);
    const w = this.worker;
    this.worker = null;
    this.parseCount = 0;
    w.terminate().catch(() => {});
  }

  async requestParse(filePath: string, content: string): Promise<ExtractionResult> {
    if (!this.WorkerClass) {
      return extractFromSource(
        filePath,
        content,
        detectLanguage(filePath, content),
        this.frameworkNames
      );
    }

    if (this.parseCount >= WORKER_RECYCLE_INTERVAL) {
      this.recycleWorker();
    }

    const worker = await this.ensureWorker();
    const id = this.nextId++;
    this.parseCount++;

    const timeoutMs = PARSE_TIMEOUT_MS + Math.floor(content.length / 100_000) * 10_000;

    return new Promise<ExtractionResult>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        this.log(`TIMEOUT: ${filePath} exceeded ${timeoutMs}ms — killing worker`);
        this.worker = null;
        this.parseCount = 0;
        reject(new Error(`Parse timed out after ${timeoutMs}ms`));
        worker.terminate().catch(() => {});
      }, timeoutMs);

      this.pending.set(id, { resolve, reject, timer });
      worker.postMessage({ type: 'parse', id, filePath, content, frameworkNames: this.frameworkNames });
    });
  }

  terminate(): void {
    if (this.worker) {
      this.worker.terminate().catch(() => {});
      this.worker = null;
    }
  }

  get hasWorker(): boolean {
    return this.WorkerClass !== null;
  }
}
