/**
 * File Scanner
 *
 * Scans directories for source files, respecting .gitignore and config
 * include/exclude patterns. Uses git ls-files for fast scanning in git repos,
 * falls back to filesystem walk for non-git projects.
 */

import * as fs from 'fs';
import * as path from 'path';
import { execFileSync } from 'child_process';
import { CodeGraphConfig } from '../types';
import { normalizePath } from '../utils';
import { logDebug } from '../errors';
import picomatch from 'picomatch';

const CODEGRAPH_IGNORE_MARKER = '.codegraphignore';

function matchesGlob(filePath: string, pattern: string): boolean {
  filePath = normalizePath(filePath);
  return picomatch.isMatch(filePath, pattern, { dot: true });
}

export function shouldIncludeFile(
  filePath: string,
  config: CodeGraphConfig
): boolean {
  for (const pattern of config.exclude) {
    if (matchesGlob(filePath, pattern)) {
      return false;
    }
  }
  for (const pattern of config.include) {
    if (matchesGlob(filePath, pattern)) {
      return true;
    }
  }
  return false;
}

function collectGitFiles(repoDir: string, prefix: string, files: Set<string>): void {
  const gitOpts = { cwd: repoDir, encoding: 'utf-8' as const, timeout: 30000, maxBuffer: 50 * 1024 * 1024, stdio: ['pipe', 'pipe', 'pipe'] as ['pipe', 'pipe', 'pipe'] };

  const tracked = execFileSync('git', ['ls-files', '-c', '--recurse-submodules'], gitOpts);
  for (const line of tracked.split('\n')) {
    const trimmed = line.trim();
    if (trimmed) {
      files.add(normalizePath(prefix + trimmed));
    }
  }

  const untracked = execFileSync('git', ['ls-files', '-o', '--exclude-standard'], gitOpts);
  for (const line of untracked.split('\n')) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    if (trimmed.endsWith('/')) {
      const childDir = path.join(repoDir, trimmed);
      if (fs.existsSync(path.join(childDir, '.git'))) {
        collectGitFiles(childDir, prefix + trimmed, files);
      }
      continue;
    }
    files.add(normalizePath(prefix + trimmed));
  }
}

export function getGitVisibleFiles(rootDir: string): Set<string> | null {
  try {
    const gitRoot = execFileSync(
      'git',
      ['rev-parse', '--show-toplevel'],
      { cwd: rootDir, encoding: 'utf-8', timeout: 5000, stdio: ['pipe', 'pipe', 'pipe'] }
    ).trim();

    if (path.resolve(gitRoot) !== path.resolve(rootDir)) {
      try {
        execFileSync(
          'git',
          ['check-ignore', '-q', path.resolve(rootDir)],
          { cwd: rootDir, encoding: 'utf-8', timeout: 5000, stdio: ['pipe', 'pipe', 'pipe'] }
        );
        return null;
      } catch {
        // Not ignored — safe to use git ls-files
      }
    }

    const files = new Set<string>();
    collectGitFiles(rootDir, '', files);
    return files;
  } catch {
    return null;
  }
}

export interface GitChanges {
  modified: string[];
  added: string[];
  deleted: string[];
}

export function getGitChangedFiles(rootDir: string, config: CodeGraphConfig): GitChanges | null {
  try {
    const output = execFileSync(
      'git',
      ['status', '--porcelain', '--no-renames'],
      { cwd: rootDir, encoding: 'utf-8', timeout: 10000, stdio: ['pipe', 'pipe', 'pipe'] }
    );

    const modified: string[] = [];
    const added: string[] = [];
    const deleted: string[] = [];

    for (const line of output.split('\n')) {
      if (line.length < 4) continue;

      const statusCode = line.substring(0, 2);
      const filePath = normalizePath(line.substring(3));

      if (!shouldIncludeFile(filePath, config)) continue;

      if (statusCode === '??') {
        added.push(filePath);
      } else if (statusCode.includes('D')) {
        deleted.push(filePath);
      } else {
        modified.push(filePath);
      }
    }

    return { modified, added, deleted };
  } catch {
    return null;
  }
}

function scanDirectoryWalk(
  rootDir: string,
  config: CodeGraphConfig,
  onProgress?: (current: number, file: string) => void
): string[] {
  const files: string[] = [];
  let count = 0;
  const visitedDirs = new Set<string>();

  function walk(dir: string): void {
    let realDir: string;
    try {
      realDir = fs.realpathSync(dir);
    } catch {
      logDebug('Skipping unresolvable directory', { dir });
      return;
    }

    if (visitedDirs.has(realDir)) {
      logDebug('Skipping already-visited directory (symlink cycle)', { dir, realDir });
      return;
    }
    visitedDirs.add(realDir);

    const ignoreMarker = path.join(dir, CODEGRAPH_IGNORE_MARKER);
    if (fs.existsSync(ignoreMarker)) {
      logDebug('Skipping directory due to .codegraphignore marker', { dir });
      return;
    }

    let entries: fs.Dirent[];
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch (error) {
      logDebug('Skipping unreadable directory', { dir, error: String(error) });
      return;
    }

    for (const entry of entries) {
      const fullPath = path.join(dir, entry.name);
      const relativePath = normalizePath(path.relative(rootDir, fullPath));

      if (entry.isSymbolicLink()) {
        try {
          const realTarget = fs.realpathSync(fullPath);
          const stat = fs.statSync(realTarget);
          if (stat.isDirectory()) {
            const dirPattern = relativePath + '/';
            let excluded = false;
            for (const pattern of config.exclude) {
              if (matchesGlob(dirPattern, pattern) || matchesGlob(relativePath, pattern)) {
                excluded = true;
                break;
              }
            }
            if (!excluded) {
              walk(fullPath);
            }
          } else if (stat.isFile()) {
            if (shouldIncludeFile(relativePath, config)) {
              files.push(relativePath);
              count++;
              onProgress?.(count, relativePath);
            }
          }
        } catch {
          logDebug('Skipping dead symlink', { fullPath });
        }
        continue;
      }

      if (entry.isDirectory()) {
        let excluded = false;
        for (const pattern of config.exclude) {
          if (matchesGlob(relativePath, pattern)) {
            excluded = true;
            break;
          }
        }
        if (!excluded) {
          walk(fullPath);
        }
      } else if (entry.isFile()) {
        if (shouldIncludeFile(relativePath, config)) {
          files.push(relativePath);
          count++;
          onProgress?.(count, relativePath);
        }
      }
    }
  }

  walk(rootDir);
  return files;
}

export function scanDirectory(
  rootDir: string,
  config: CodeGraphConfig,
  onProgress?: (current: number, file: string) => void
): string[] {
  const gitFiles = getGitVisibleFiles(rootDir);
  if (gitFiles) {
    const files: string[] = [];
    let count = 0;
    for (const filePath of gitFiles) {
      if (shouldIncludeFile(filePath, config)) {
        files.push(filePath);
        count++;
        onProgress?.(count, filePath);
      }
    }
    return files;
  }

  return scanDirectoryWalk(rootDir, config, onProgress);
}

export async function scanDirectoryAsync(
  rootDir: string,
  config: CodeGraphConfig,
  onProgress?: (current: number, file: string) => void
): Promise<string[]> {
  const gitFiles = getGitVisibleFiles(rootDir);
  if (gitFiles) {
    const files: string[] = [];
    let count = 0;
    for (const filePath of gitFiles) {
      if (shouldIncludeFile(filePath, config)) {
        files.push(filePath);
        count++;
        onProgress?.(count, filePath);
        if (count % 100 === 0) {
          await new Promise<void>(r => setImmediate(r));
        }
      }
    }
    return files;
  }

  return scanDirectoryWalk(rootDir, config, onProgress);
}
