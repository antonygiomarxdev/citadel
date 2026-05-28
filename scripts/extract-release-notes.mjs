#!/usr/bin/env node
/**
 * Extract a release-notes block from CHANGELOG.md for a given version
 * (or unwrap text supplied on stdin), then join hard-wrapped paragraphs.
 *
 * Why: GitHub renders release-note Markdown with GFM hard breaks, so
 * every `\n` becomes `<br>`. The CHANGELOG is hard-wrapped at ~75
 * chars for readable diffs, which then renders as awkward visible
 * line breaks on the release page. This script joins indented
 * continuation lines into a single line per bullet so the GFM
 * renderer produces clean paragraphs.
 *
 * Usage:
 *   extract-release-notes.mjs <version>     # read CHANGELOG.md
 *   extract-release-notes.mjs --stdin       # read from stdin (any text)
 */

import { readFileSync } from 'fs';

const arg = process.argv[2];
if (!arg) {
  console.error('usage: extract-release-notes.mjs <version> | --stdin');
  process.exit(1);
}

let block;
if (arg === '--stdin') {
  block = readFileSync(0, 'utf8').replace(/\r\n?/g, '\n').split('\n');
} else {
  const version = arg;
  const escaped = version.replace(/\./g, '\\.');
  const headerRe = new RegExp(`^## \\[${escaped}\\]`);
  const anyHeaderRe = /^## \[/;
  const lines = readFileSync('CHANGELOG.md', 'utf8').split('\n');
  const start = lines.findIndex((l) => headerRe.test(l));
  if (start === -1) {
    console.error(`no '## [${version}]' entry found in CHANGELOG.md`);
    process.exit(1);
  }
  const after = lines.findIndex((l, i) => i > start && anyHeaderRe.test(l));
  block = lines.slice(start, after === -1 ? lines.length : after);
}

const out = [];
let buf = '';
let stack = [];

function flushBuf() {
  if (buf !== '') {
    out.push(buf);
    buf = '';
  }
}

function leadingSpaces(s) {
  const m = s.match(/^(\s*)/);
  return m ? m[1].length : 0;
}

const listItemRe = /^(\s*)([-*]|\d+\.)\s+/;
const fenceRe = /^\s*```/;

let inFence = false;

for (const line of block) {
  if (fenceRe.test(line)) {
    flushBuf();
    stack = [];
    out.push(line);
    inFence = !inFence;
    continue;
  }
  if (inFence) {
    out.push(line);
    continue;
  }
  if (/^\s*$/.test(line)) {
    flushBuf();
    out.push('');
    continue;
  }
  if (/^#/.test(line)) {
    flushBuf();
    stack = [];
    out.push(line);
    continue;
  }
  const itemMatch = line.match(listItemRe);
  if (itemMatch) {
    flushBuf();
    const indent = itemMatch[1].length;
    while (stack.length > 0 && stack[stack.length - 1].indent >= indent) {
      stack.pop();
    }
    stack.push({ indent });
    buf = line;
    continue;
  }
  if (/^\s/.test(line)) {
    const indent = leadingSpaces(line);
    while (stack.length > 1 && stack[stack.length - 1].indent >= indent) {
      flushBuf();
      stack.pop();
    }
    const trimmed = line.replace(/^\s+/, '');
    buf = buf === '' ? trimmed : `${buf} ${trimmed}`;
    continue;
  }
  flushBuf();
  stack = [];
  out.push(line);
}
flushBuf();

process.stdout.write(out.join('\n'));
if (!out[out.length - 1]?.endsWith('\n')) process.stdout.write('\n');
