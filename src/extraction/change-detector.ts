/**
 * Change Detector
 *
 * Compares the filesystem against the database to find added, modified,
 * and removed files. Uses `git status` as a fast path, falling back to
 * a full filesystem scan when git is unavailable.
 */

import * as path from 'path';
import * as fs from 'fs';
import type { FileRecord } from '../types';
import { logDebug } from '../errors';
import { hashContent } from '../utils/hash';
import {
  scanDirectory,
  getGitChangedFiles,
} from './file-scanner';
import type { CodeGraphConfig } from '../types';

export interface FileChanges {
  added: string[];
  modified: string[];
  removed: string[];
}

export interface ChangeDetectorOps {
  getFileByPath: (filePath: string) => FileRecord | null | undefined;
  getAllFiles: () => FileRecord[];
}

export function detectFileChanges(
  rootDir: string,
  config: CodeGraphConfig,
  db: ChangeDetectorOps
): FileChanges {
  const gitChanges = getGitChangedFiles(rootDir, config);

  if (gitChanges) {
    return detectFromGitStatus(gitChanges.deleted, [...gitChanges.modified, ...gitChanges.added], rootDir, db);
  }

  return detectFromFullScan(rootDir, config, db);
}

function detectFromGitStatus(
  deleted: string[],
  candidates: string[],
  rootDir: string,
  db: ChangeDetectorOps
): FileChanges {
  const added: string[] = [];
  const modified: string[] = [];
  const removed: string[] = [];

  for (const filePath of deleted) {
    const tracked = db.getFileByPath(filePath);
    if (tracked) {
      removed.push(filePath);
    }
  }

  for (const filePath of candidates) {
    const fullPath = path.join(rootDir, filePath);
    let content: string;
    try {
      content = fs.readFileSync(fullPath, 'utf-8');
    } catch (error) {
      logDebug('Skipping unreadable file during change detection', { filePath, error: String(error) });
      continue;
    }

    const fileHash = hashContent(content);
    const tracked = db.getFileByPath(filePath);

    if (!tracked) {
      added.push(filePath);
    } else if (tracked.contentHash !== fileHash) {
      modified.push(filePath);
    }
  }

  return { added, modified, removed };
}

function detectFromFullScan(
  rootDir: string,
  config: CodeGraphConfig,
  db: ChangeDetectorOps
): FileChanges {
  const currentFiles = new Set(scanDirectory(rootDir, config));
  const trackedFiles = db.getAllFiles();

  const trackedMap = new Map<string, FileRecord>();
  for (const f of trackedFiles) {
    trackedMap.set(f.path, f);
  }

  const added: string[] = [];
  const modified: string[] = [];
  const removed: string[] = [];

  for (const tracked of trackedFiles) {
    if (!currentFiles.has(tracked.path)) {
      removed.push(tracked.path);
    }
  }

  for (const filePath of currentFiles) {
    const fullPath = path.join(rootDir, filePath);
    let content: string;
    try {
      content = fs.readFileSync(fullPath, 'utf-8');
    } catch (error) {
      logDebug('Skipping unreadable file during change detection', { filePath, error: String(error) });
      continue;
    }

    const fileHash = hashContent(content);
    const tracked = trackedMap.get(filePath);

    if (!tracked) {
      added.push(filePath);
    } else if (tracked.contentHash !== fileHash) {
      modified.push(filePath);
    }
  }

  return { added, modified, removed };
}
