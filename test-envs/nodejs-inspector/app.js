/**
 * NestJS-like stress app for rtop Node.js profiling validation.
 * Exposes:
 *   GET /      → healthy echo
 *   GET /block → synchronous CPU spin (triggers ELU alert)
 *   GET /leak  → allocates 10MB to heap each request (triggers OOM RISK)
 *   GET /gc    → forces a major GC cycle
 *
 * Run with: node --inspect=0.0.0.0:9229 app.js
 */

const http = require('http');
const v8 = require('v8');

const LEAK_STORE = [];

const server = http.createServer((req, res) => {
  const url = req.url;

  if (url === '/') {
    res.writeHead(200);
    res.end(JSON.stringify({ status: 'ok', heap: process.memoryUsage() }));
    return;
  }

  if (url === '/block') {
    // Sync CPU spin ~200ms → event loop delay spike
    const start = Date.now();
    while (Date.now() - start < 200) { /* spin */ }
    res.writeHead(200);
    res.end(JSON.stringify({ blocked_ms: 200 }));
    return;
  }

  if (url === '/leak') {
    // Allocate 10MB and keep reference → heap grows
    const buf = Buffer.alloc(10 * 1024 * 1024, 'x');
    LEAK_STORE.push(buf);
    res.writeHead(200);
    res.end(JSON.stringify({ leaked_mb: LEAK_STORE.length * 10, total_refs: LEAK_STORE.length }));
    return;
  }

  if (url === '/gc') {
    if (global.gc) {
      global.gc();
      res.writeHead(200);
      res.end(JSON.stringify({ gc: 'triggered' }));
    } else {
      res.writeHead(200);
      res.end(JSON.stringify({ gc: 'not available (run with --expose-gc)' }));
    }
    return;
  }

  res.writeHead(404);
  res.end('not found');
});

server.listen(3000, () => {
  console.log('[rtop-test-node] Server listening on :3000');
  console.log('[rtop-test-node] V8 Inspector on :9229');
  console.log('[rtop-test-node] Endpoints: / /block /leak /gc');

  // Periodic mild ELU noise to keep event loop busy
  setInterval(() => {
    const arr = new Array(10000).fill(0).map((_, i) => i * Math.random());
    arr.sort();
  }, 500);
});
