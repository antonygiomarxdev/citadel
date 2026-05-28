const { CodeGraph } = require('../../dist');
const fs = require('fs');
const path = require('path');
const os = require('os');

const ITERATIONS = 3;
const ROOT = path.resolve(__dirname, '../..');

// Ensure .codegraph exists in root
if (!require('fs').existsSync(path.join(ROOT, '.codegraph'))) {
  console.error('Run codegraph init first');
  process.exit(1);
}

function sec(ms) { return (ms / 1000).toFixed(1); }

async function runOnce() {
  const cg = await CodeGraph.open(ROOT);

  const start = process.hrtime.bigint();
  const result = await cg.indexAll({ onProgress: () => {} });
  const wallMs = Number(process.hrtime.bigint() - start) / 1_000_000;

  cg.close();
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

  const times = [];
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
