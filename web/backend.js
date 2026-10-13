/* gers-lr demo — backend selection.
 *
 * The page talks to four calls: stats, network, find, resolve. They are answered
 * either by `gers-lr serve` (HTTP JSON API on the same origin) or, when the page
 * is hosted statically, by the very same Rust resolver compiled to WebAssembly
 * and running in this tab. Same code, same answers; only the transport differs.
 */
window.GersBackend = (() => {
  const PACK = 'seville.pack.gz';
  const mb = (n) => (n / 1e6).toFixed(1);

  async function detect(status = () => {}) {
    try {
      const r = await fetch('api/stats', { cache: 'no-store' });
      if (r.ok && (r.headers.get('content-type') || '').includes('json')) return http();
    } catch { /* no server here: fall through to WebAssembly */ }
    return wasm(status);
  }

  function http() {
    const get = async (path) => {
      const r = await fetch(path);
      const d = await r.json();
      if (!r.ok) throw new Error(d.error || r.statusText);
      return d;
    };
    return {
      kind: 'http',
      label: 'answered by gers-lr serve (HTTP)',
      stats: () => get('api/stats'),
      network: () => get('api/network'),
      find: (q) => get('api/find?q=' + encodeURIComponent(q)),
      resolve: (params) => get('api/resolve?' + new URLSearchParams(params)),
    };
  }

  async function wasm(status) {
    const t0 = performance.now();
    status('loading the resolver…');
    const [mod, bytes] = await Promise.all([
      import('./pkg/gers_lr.js').then(async (m) => { await m.default(); return m; }),
      fetchPack(status),
    ]);
    const gers = new mod.Gers(bytes);
    const secs = ((performance.now() - t0) / 1000).toFixed(1);
    status(`ready in ${secs} s`);
    const json = (s) => JSON.parse(s);
    return {
      kind: 'wasm',
      label: `answered in your browser · Rust → WebAssembly · ${gers.source()} · ${secs} s to load`,
      stats: async () => json(gers.stats()),
      network: async () => json(gers.network()),
      find: async (q) => json(gers.find(q)),
      resolve: async (params) =>
        json(gers.resolve(JSON.stringify(Object.fromEntries(new URLSearchParams(params))))),
    };
  }

  async function fetchPack(status) {
    const r = await fetch(PACK);
    if (!r.ok) throw new Error(`${PACK}: HTTP ${r.status}`);
    const total = Number(r.headers.get('content-length')) || 0;
    let got = 0;
    const progress = new TransformStream({
      transform(chunk, ctrl) {
        got += chunk.byteLength;
        status(`loading Seville… ${mb(got)}${total ? ' / ' + mb(total) : ''} MB`);
        ctrl.enqueue(chunk);
      },
    });
    let bytes = new Uint8Array(await new Response(r.body.pipeThrough(progress)).arrayBuffer());
    // Static hosts serve the .gz as-is; some set Content-Encoding and the browser
    // has already inflated it. Sniff the gzip magic instead of guessing.
    if (bytes[0] === 0x1f && bytes[1] === 0x8b) {
      status('unpacking…');
      const inflated = new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip'));
      bytes = new Uint8Array(await new Response(inflated).arrayBuffer());
    }
    return bytes;
  }

  return { detect };
})();
