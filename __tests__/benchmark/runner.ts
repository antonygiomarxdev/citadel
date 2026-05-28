import { CodeGraph } from '../../dist';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const ITERATIONS = 3;
const ROOT = path.resolve(__dirname, '../..');

function sec(ms: number): string {
  return (ms / 1000).toFixed(1);
}

async function runOnce(): Promise<{ wallMs: number; filesIndexed: number; filesErrored: number; nodes: number; edges: number }> {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'cg-bench-'));
  const cg = await CodeGraph.init(tmp, { maxFileSize: 10 * 1024 * 1024 });

  const start = process.hrtime.bigint();
  const result = await cg.indexAll({
    onProgress: () => {},
  });
  const wallMs = Number(process.hrtime.bigint() - start) / 1_000_000;

  cg.close();
  fs.rmSync(tmp, { recursive: true, force: true });

  return {
    wallMs,
    filesIndexed: result.filesIndexed,
    filesErrored: result.filesErrored,
    nodes: result.nodesCreated,
    edges: result.edgesCreated,
  };
}

async function main() {
  const info = `Benchmark: ${ROOT}
Iterations: ${ITERATIONS}
Node: ${process.version}
CPU: ${os.cpus()[0].model} (${os.cpus().length} cores)
RAM: ${(os.totalmem() / 1024 / 1024 / 1024).toFixed(1)} GB
`;

  console.log(info);

  const times: number[] = [];
  let totalFiles = 0, totalNodes = 0, totalEdges = 0;

  for (let i = 0; i < ITERATIONS; i++) {
    const r = await runOnce();
    times.push(r.wallMs);
    totalFiles += r.filesIndexed;
    totalNodes += r.nodes;
    totalEdges += r.edges;
    console.log(`  Run ${i + 1}: ${sec(r.wallMs)}s  (${r.filesIndexed} files, ${r.nodes} nodes, ${r.edges} edges)`);
    if (global.gc) global.gc();
    await new Promise(r => setTimeout(r, 500));
  }

  const avg = times.reduce((a, b) => a + b, 0) / times.length;
  const min = Math.min(...times);
  const max = Math.max(...times);

  console.log('');
  console.log(`  Avg: ${sec(avg)}s  Min: ${sec(min)}s  Max: ${sec(max)}s`);
  console.log(`  Files/sec: ${(totalFiles / ITERATIONS / (avg / 1000)).toFixed(0)}`);
  console.log(`  Nodes/sec: ${(totalNodes / ITERATIONS / (avg / 1000)).toFixed(0)}`);
}

main().catch(console.error);
