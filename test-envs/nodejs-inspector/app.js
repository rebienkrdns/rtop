/**
 * rtop Node.js V8 Telemetry - entorno de validación continua.
 *
 * Genera actividad automática en TODAS las métricas para validar el panel rtop:
 *   • ELU / Event Loop Delay  →  CPU spin periódico moderado
 *   • Heap Used / Spaces      →  allocaciones en ciclo con liberación (genera GC)
 *   • Minor GC / Major GC     →  rastreado via PerformanceObserver
 *   • Libuv Handles           →  varios servidores TCP + timers permanentes
 *   • Thread Pool             →  operaciones de crypto en libuv threadpool
 *
 * HTTP endpoints (opcionales para stress manual adicional):
 *   GET /        → estado + métricas en JSON
 *   GET /block   → CPU spin 200ms (dispara ELU alert)
 *   GET /leak    → acumula 20MB (dispara OOM RISK)
 *   GET /gc      → fuerza GC explícito
 *   GET /reset   → libera el heap acumulado
 */

const http = require('http');
const net  = require('net');
const crypto = require('crypto');

// ─── GC tracking via PerformanceObserver ──────────────────────────────────────
const gcStats = {
  minorCount: 0, minorTotalMs: 0, minorLastMs: 0,
  majorCount: 0, majorTotalMs: 0, majorLastMs: 0,
  startTime: Date.now(),
};
global.__rtop_gc = gcStats;

try {
  const { PerformanceObserver } = require('perf_hooks');
  const obs = new PerformanceObserver((list) => {
    list.getEntries().forEach((e) => {
      if (e.entryType !== 'gc') return;
      const isMinor = e.detail && (e.detail.kind === 1 || e.detail.kind === 4);
      if (isMinor) {
        gcStats.minorCount++;
        gcStats.minorTotalMs += e.duration;
        gcStats.minorLastMs = e.duration;
      } else {
        gcStats.majorCount++;
        gcStats.majorTotalMs += e.duration;
        gcStats.majorLastMs = e.duration;
      }
    });
  });
  obs.observe({ entryTypes: ['gc'] });
  console.log('[rtop] GC observer activo');
} catch (e) {
  console.log('[rtop] PerformanceObserver no disponible:', e.message);
}

// ─── Libuv handles permanentes ────────────────────────────────────────────────
// 5 servidores TCP internos que se mantienen abiertos → active handles > 0
const INTERNAL_SERVERS = [];
for (let i = 0; i < 5; i++) {
  const srv = net.createServer();
  srv.listen(0); // puerto aleatorio
  INTERNAL_SERVERS.push(srv);
}
console.log(`[rtop] ${INTERNAL_SERVERS.length} servidores TCP internos abiertos (Libuv handles)`);

// ─── Heap churn automático (genera Minor + Major GC) ─────────────────────────
const HEAP_RING = new Array(30).fill(null);
let ringIdx = 0;
let heapChurnActive = true;

setInterval(() => {
  if (!heapChurnActive) return;
  // Alterna entre bloques de 500KB y 2MB para ejercitar new_space y old_space
  const size = ringIdx % 5 === 0 ? 2 * 1024 * 1024 : 512 * 1024;
  HEAP_RING[ringIdx % HEAP_RING.length] = Buffer.alloc(size, ringIdx & 0xff);
  ringIdx++;
}, 300);

// GC explícito cada 8 segundos si está disponible
if (global.gc) {
  setInterval(() => {
    global.gc();
  }, 8000);
}

// ─── ELU stress moderado ──────────────────────────────────────────────────────
// CPU spin de 5–15ms cada 800ms → ELU sube a 1–3% de forma estable
setInterval(() => {
  const spinMs = 5 + Math.floor(Math.random() * 10);
  const end = Date.now() + spinMs;
  while (Date.now() < end) { Math.sqrt(Math.random()); }
}, 800);

// ─── Thread Pool activity (crypto) ───────────────────────────────────────────
// pbkdf2 usa el libuv thread pool → threadpool saturation
setInterval(() => {
  crypto.pbkdf2('password', 'salt', 1000, 32, 'sha256', () => {});
}, 1500);

// ─── Store manual leaks ───────────────────────────────────────────────────────
const LEAK_STORE = [];

// ─── HTTP server ──────────────────────────────────────────────────────────────
const server = http.createServer((req, res) => {
  if (req.url === '/') {
    const m = process.memoryUsage();
    const elu = (typeof performance !== 'undefined' && performance.eventLoopUtilization)
      ? performance.eventLoopUtilization()
      : { utilization: 0 };
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({
      status: 'ok',
      heapUsedMB: (m.heapUsed / 1024 / 1024).toFixed(1),
      heapTotalMB: (m.heapTotal / 1024 / 1024).toFixed(1),
      elu_pct: (elu.utilization * 100).toFixed(1),
      gc: {
        minor: gcStats.minorCount,
        major: gcStats.majorCount,
      },
      handles: process._getActiveHandles ? process._getActiveHandles().length : '?',
    }));
    return;
  }

  if (req.url === '/block') {
    // Spin 300ms → ELU alert (>85% por un instante)
    const start = Date.now();
    while (Date.now() - start < 300) { /* CPU spin */ }
    res.writeHead(200);
    res.end(JSON.stringify({ blocked_ms: 300 }));
    return;
  }

  if (req.url === '/leak') {
    LEAK_STORE.push(Buffer.alloc(20 * 1024 * 1024, 0xff));
    res.writeHead(200);
    res.end(JSON.stringify({ leak_refs: LEAK_STORE.length, total_mb: LEAK_STORE.length * 20 }));
    return;
  }

  if (req.url === '/reset') {
    heapChurnActive = true;
    LEAK_STORE.length = 0;
    if (global.gc) global.gc();
    res.writeHead(200);
    res.end(JSON.stringify({ reset: true }));
    return;
  }

  if (req.url === '/gc') {
    if (global.gc) { global.gc(); res.end(JSON.stringify({ gc: 'triggered' })); }
    else res.end(JSON.stringify({ gc: 'not available (run with --expose-gc)' }));
    return;
  }

  res.writeHead(404);
  res.end('not found');
});

server.listen(3000, () => {
  console.log('[rtop-test] HTTP en :3000');
  console.log('[rtop-test] V8 Inspector en :9229');
  console.log('[rtop-test] Actividad automática: heap churn cada 300ms, ELU spin cada 800ms, crypto cada 1500ms');
  console.log('[rtop-test] Endpoints: / /block /leak /reset /gc');
});
