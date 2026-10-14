/* overrules explorer: click a road, describe yourself, read the verdict and why. */

const ACCENT = '#c6ff4d';
const MODES = ['car', 'truck', 'hgv', 'bus', 'motorcycle', 'bicycle', 'foot', 'emergency', 'hov', 'motor_vehicle', 'vehicle'];
const USING = ['as_customer', 'at_destination', 'to_deliver', 'to_farm', 'for_forestry'];
const RECOGNIZED = ['as_permitted', 'as_private', 'as_disabled', 'as_employee', 'as_student'];
const STORIES = [
  { label: 'Ring road: 60 one way, 80 the other', segment: '99698abc', at: 0.9, heading: 'forward', mode: 'car' },
  { label: 'Rule #0 says 90 — it\u2019s 80 or 50', segment: 'e26d3384', at: 0.05, heading: 'forward', mode: 'car' },
  { label: 'Living street: unless you live there', segment: '1b946971', at: 0.5, heading: 'backward', mode: 'car' },
  { label: '3.5 t limit, 12 t lorry', segment: '645f3ec7', at: 0.5, heading: 'forward', mode: 'truck', vehicle: { weight: 12 } },
  { label: '17 banned turns', segment: 'a022afe5', at: 0.5, heading: 'forward', mode: 'car' },
  { label: 'Planar maths is 50 m off (SE-40)', segment: '55c99d8b', at: 0.6562, heading: 'forward', mode: 'car' },
];

const state = {
  segment: null, at: null,
  heading: 'forward', mode: 'car', time: '',
  using: new Set(), recognized: new Set(), vehicle: {},
};
let map, marker, current = null, lastSegment = null, pending = null;

const $ = (s) => document.querySelector(s);
const $$ = (s) => document.querySelectorAll(s);
const el = (tag, cls, html) => { const e = document.createElement(tag); if (cls) e.className = cls; if (html != null) e.innerHTML = html; return e; };
const fmt = (n, d = 1) => Number(n).toFixed(d);
const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const empty = () => ({ type: 'FeatureCollection', features: [] });

/* ---------- backend ---------- */
// HTTP API when served by `overrules serve`, otherwise the resolver as WebAssembly (see backend.js).
const backend = OverrulesBackend.detect((msg) => { $('#subtitle').textContent = msg; })
  .then((b) => { $('#backend').textContent = b.label; return b; })
  .catch((e) => { showError(`The resolver could not start: ${e.message}`); throw e; });
const api = async (call, arg) => (await backend)[call](arg);

/* ---------- map ---------- */
// Embedded webviews can report the document as hidden, which pauses requestAnimationFrame
// and stalls MapLibre before its first frame. Fall back to a timer while hidden.
(() => {
  const raf = window.requestAnimationFrame.bind(window), caf = window.cancelAnimationFrame.bind(window);
  window.requestAnimationFrame = (cb) => document.hidden ? -setTimeout(() => cb(performance.now()), 16) : raf(cb);
  window.cancelAnimationFrame = (id) => (id < 0 ? clearTimeout(-id) : caf(id));
})();

function initMap() {
  map = new maplibregl.Map({
    container: 'map',
    style: 'https://tiles.openfreemap.org/styles/dark',
    center: [-5.985, 37.39], zoom: 12.4, pitch: 0,
    attributionControl: { compact: true },
  });
  map.addControl(new maplibregl.NavigationControl({ showCompass: false }), 'top-right');

  map.on('load', async () => {
    const net = await api('network');
    map.addSource('net', { type: 'geojson', data: net, promoteId: 'id' });
    map.addLayer({
      id: 'net', type: 'line', source: 'net',
      layout: { 'line-cap': 'round', 'line-join': 'round' },
      paint: {
        'line-color': ['case',
          ['>=', ['get', 'k'], 8], 'rgba(198,255,77,0.55)',   // has `between` boundaries
          ['>', ['get', 'k'], 0], 'rgba(255,255,255,0.30)',   // has any scoped rule
          'rgba(255,255,255,0.10)'],
        'line-width': ['interpolate', ['linear'], ['zoom'], 11, 0.7, 14, 1.6, 17, 3.2],
      },
    });
    map.addLayer({
      id: 'net-hover', type: 'line', source: 'net', filter: ['==', ['get', 'id'], ''],
      layout: { 'line-cap': 'round', 'line-join': 'round' },
      paint: { 'line-color': '#ffffff', 'line-width': 4, 'line-opacity': 0.55 },
    });

    map.addSource('sel', { type: 'geojson', data: empty() });
    map.addLayer({
      id: 'sel-halo', type: 'line', source: 'sel', filter: ['==', ['geometry-type'], 'LineString'],
      layout: { 'line-cap': 'round', 'line-join': 'round' },
      paint: { 'line-color': ACCENT, 'line-width': 14, 'line-opacity': 0.18, 'line-blur': 6 },
    });
    map.addLayer({
      id: 'sel-line', type: 'line', source: 'sel', filter: ['==', ['geometry-type'], 'LineString'],
      layout: { 'line-cap': 'round', 'line-join': 'round' },
      paint: { 'line-color': ACCENT, 'line-width': 4.5 },
    });
    map.addLayer({
      id: 'sel-ghost', type: 'circle', source: 'sel', filter: ['==', ['get', 'kind'], 'ghost'],
      paint: { 'circle-radius': 3.5, 'circle-color': '#ff5d5d', 'circle-opacity': 0.9 },
    });
    map.addLayer({
      id: 'sel-bound', type: 'circle', source: 'sel', filter: ['==', ['get', 'kind'], 'boundary'],
      paint: { 'circle-radius': 4.5, 'circle-color': '#0b0d10', 'circle-stroke-color': ACCENT, 'circle-stroke-width': 2 },
    });
    map.addLayer({
      id: 'sel-turn', type: 'circle', source: 'sel', filter: ['==', ['get', 'kind'], 'turn'],
      paint: { 'circle-radius': 7, 'circle-color': '#ff5d5d', 'circle-stroke-color': '#0b0d10', 'circle-stroke-width': 2 },
    });
    map.addLayer({
      id: 'sel-conn', type: 'circle', source: 'sel', filter: ['==', ['get', 'kind'], 'connector'],
      paint: { 'circle-radius': 3, 'circle-color': '#ffffff', 'circle-opacity': 0.8 },
    });

    const tip = $('#tooltip');
    map.on('mousemove', 'net', (e) => {
      const f = e.features[0];
      map.setFilter('net-hover', ['==', ['get', 'id'], f.properties.id]);
      map.getCanvas().classList.add('hovering');
      tip.hidden = false;
      tip.style.left = e.point.x + 'px'; tip.style.top = e.point.y + 'px';
      tip.innerHTML = `${esc(f.properties.n || 'unnamed')} <span class="m">${esc(f.properties.c)}${f.properties.k ? ' · rules' : ''}</span>`;
    });
    map.on('mouseleave', 'net', () => {
      map.setFilter('net-hover', ['==', ['get', 'id'], '']);
      map.getCanvas().classList.remove('hovering');
      tip.hidden = true;
    });
    map.on('click', (e) => resolve({ lon: e.lngLat.lng, lat: e.lngLat.lat }));

    for (const [k, v] of Object.entries(pointLayersTips())) {
      map.on('mouseenter', k, (e) => { tip.hidden = false; tip.innerHTML = v(e.features[0].properties); tip.style.left = e.point.x + 'px'; tip.style.top = e.point.y + 'px'; });
      map.on('mouseleave', k, () => { tip.hidden = true; });
    }

    const s = await api('stats');
    $('#subtitle').textContent = `Seville · ${s.segments.toLocaleString()} segments · ${(s.speed_rules + s.access_rules + s.turn_rules).toLocaleString()} rules`;
  });

  const arrow = el('div', 'arrow');
  arrow.innerHTML = `<svg viewBox="0 0 34 34" width="34" height="34"><circle cx="17" cy="17" r="8" fill="#0b0d10" stroke="${ACCENT}" stroke-width="2.5"/><path d="M17 1 L23 11 L17 8.5 L11 11 Z" fill="${ACCENT}"/></svg>`;
  marker = new maplibregl.Marker({ element: arrow, rotationAlignment: 'map', pitchAlignment: 'map' });
}

function pointLayersTips() {
  return {
    'sel-bound': (p) => `rule boundary at ${fmt(p.fraction, 4)} <span class="m">geodesic</span>`,
    'sel-ghost': (p) => `planar maths would put it here <span class="m">${fmt(p.err, 1)} m off</span>`,
    'sel-turn': (p) => `banned turn #${p.index} <span class="m">onto ${esc(p.target)}</span>`,
    'sel-conn': (p) => `connector <span class="m">at ${fmt(p.at, 3)}</span>`,
  };
}

/* ---------- controls ---------- */
function initControls() {
  const mode = $('#mode');
  for (const m of MODES) mode.append(new Option(m.replace('_', ' '), m));
  mode.value = state.mode;
  mode.onchange = () => { state.mode = mode.value; rerun(); };

  for (const b of $('#heading').querySelectorAll('button')) {
    b.onclick = () => { state.heading = b.dataset.h; syncHeading(); rerun(); };
  }
  chips($('#using'), USING, state.using);
  chips($('#recognized'), RECOGNIZED, state.recognized);
  for (const i of $('#vehicle').querySelectorAll('input[data-k]')) {
    i.onchange = () => { if (i.value === '') delete state.vehicle[i.dataset.k]; else state.vehicle[i.dataset.k] = i.value; rerun(); };
  }
  $('#time').onchange = (e) => { state.time = e.target.value; rerun(); };

  const at = $('#at');
  at.oninput = () => { if (state.segment) resolve({ segment: state.segment, at: Number(at.value) }); };

  const stories = $('#stories');
  for (const s of STORIES) {
    const b = el('button', null, esc(s.label));
    b.dataset.segment = s.segment;
    b.onclick = () => applyStory(s);
    stories.append(b);
  }

  const q = $('#q'), res = $('#results');
  let t;
  q.oninput = () => { clearTimeout(t); t = setTimeout(search, 120); };
  q.onfocus = () => { if (res.children.length) res.hidden = false; };
  document.addEventListener('click', (e) => { if (!e.target.closest('.search')) res.hidden = true; });
  async function search() {
    const v = q.value.trim();
    if (v.length < 2) { res.hidden = true; res.innerHTML = ''; return; }
    const hits = await api('find', v);
    res.innerHTML = '';
    for (const h of hits) {
      const li = el('li', null,
        `<span class="n">${esc(h.name || 'unnamed')} <span class="m">${esc(h.class)}</span></span>` +
        `<span class="m">${fmt(h.length_m, 0)} m · ${h.speed}s ${h.access}a ${h.turns}t</span>`);
      li.onclick = () => { res.hidden = true; q.value = h.name || h.id; resolve({ segment: h.id, at: 0.5 }); };
      res.append(li);
    }
    res.hidden = hits.length === 0;
  }
}

function chips(container, values, set) {
  for (const v of values) {
    const b = el('button', 'chip', esc(v));
    b.onclick = () => { if (set.has(v)) set.delete(v); else set.add(v); b.classList.toggle('on', set.has(v)); rerun(); };
    container.append(b);
  }
}

function syncHeading() {
  for (const b of $('#heading').querySelectorAll('button')) b.classList.toggle('on', b.dataset.h === state.heading);
}

function syncControls() {
  $('#mode').value = state.mode;
  syncHeading();
  for (const c of $('#using').children) c.classList.toggle('on', state.using.has(c.textContent));
  for (const c of $('#recognized').children) c.classList.toggle('on', state.recognized.has(c.textContent));
  for (const i of $('#vehicle').querySelectorAll('input[data-k]')) i.value = state.vehicle[i.dataset.k] ?? '';
  $('#time').value = state.time;
}

function applyStory(s) {
  state.heading = s.heading; state.mode = s.mode; state.time = '';
  state.using.clear(); for (const v of s.using || []) state.using.add(v);
  state.recognized.clear(); for (const v of s.recognized || []) state.recognized.add(v);
  state.vehicle = Object.assign({}, s.vehicle || {});
  syncControls();
  resolve({ segment: s.segment, at: s.at });
}

function rerun() {
  if (state.segment) resolve({ segment: state.segment, at: state.at });
}

/* ---------- resolve ---------- */
async function resolve(where) {
  const p = new URLSearchParams();
  if (where.segment) { p.set('segment', where.segment); if (where.at != null) p.set('at', where.at); }
  else { p.set('lon', where.lon); p.set('lat', where.lat); }
  p.set('heading', state.heading);
  p.set('mode', state.mode);
  if (state.time) p.set('time', state.time);
  if (state.using.size) p.set('using', [...state.using].join(','));
  if (state.recognized.size) p.set('recognized', [...state.recognized].join(','));
  const dims = Object.entries(state.vehicle).filter(([, v]) => v !== '' && v != null).map(([k, v]) => `${k}=${v}`);
  if (dims.length) p.set('vehicle', dims.join(','));

  const key = p.toString();
  pending = key;
  let data;
  try { data = await api('resolve', p); }
  catch (e) { if (pending === key) showError(e.message); return; }
  if (pending !== key) return;              // a newer request superseded this one
  current = data;
  state.segment = data.segment.id;
  state.at = data.position?.fraction ?? null;
  render(data);
}

function showError(msg) {
  const q = $('#question');
  q.hidden = false;
  q.className = 'question error';
  q.innerHTML = `<p>${esc(msg)}</p>`;
}

/* ---------- render ---------- */
function render(d) {
  const seg = d.segment, pos = d.position;
  const isNew = seg.id !== lastSegment;
  lastSegment = seg.id;

  // map
  const feats = [{ type: 'Feature', properties: { kind: 'segment' }, geometry: seg.geometry }];
  for (const b of d.boundaries) {
    feats.push({ type: 'Feature', properties: { kind: 'boundary', fraction: b.fraction }, geometry: { type: 'Point', coordinates: [b.lon, b.lat] } });
    if (b.planar_error_m > 0.5) feats.push({ type: 'Feature', properties: { kind: 'ghost', err: b.planar_error_m }, geometry: { type: 'Point', coordinates: [b.planar_lon, b.planar_lat] } });
  }
  for (const c of seg.connectors) feats.push({ type: 'Feature', properties: { kind: 'connector', at: c.at }, geometry: { type: 'Point', coordinates: [c.lon, c.lat] } });
  const inForce = new Set(d.prohibited_transitions.in_force);
  for (const t of d.turns) {
    if (t.connector && inForce.has(t.index)) feats.push({ type: 'Feature', properties: { kind: 'turn', index: t.index, target: t.target }, geometry: { type: 'Point', coordinates: t.connector } });
  }
  map.getSource('sel').setData({ type: 'FeatureCollection', features: feats });

  if (pos) {
    marker.setLngLat([pos.lon, pos.lat]).setRotation(state.heading === 'forward' ? pos.bearing_forward_deg : (pos.bearing_forward_deg + 180) % 360).addTo(map);
  } else {
    marker.remove();
  }
  if (isNew) {
    const q = $('#q');
    if (q.value && q.value !== (seg.name || seg.id)) { q.value = ''; $('#results').hidden = true; }
    const xs = seg.geometry.coordinates.map((c) => c[0]), ys = seg.geometry.coordinates.map((c) => c[1]);
    map.fitBounds([[Math.min(...xs), Math.min(...ys)], [Math.max(...xs), Math.max(...ys)]],
      { padding: { left: 480, top: 90, right: 90, bottom: 90 }, maxZoom: 17.2, duration: 900 });
  }

  // heading compass labels
  if (pos) {
    $('#heading [data-h=forward] small').textContent = compassShort(pos.bearing_forward_deg);
    $('#heading [data-h=backward] small').textContent = compassShort(pos.bearing_forward_deg + 180);
  }

  // question
  const qEl = $('#question');
  qEl.hidden = false;
  qEl.className = 'question';
  qEl.innerHTML = `<p>${question(d)}</p>`;
  for (const b of $$('#stories button')) b.classList.toggle('on', seg.id.startsWith(b.dataset.segment));

  // verdict tiles
  $('#answer').hidden = false;
  tile('#t-speed', speedTile(d.speed_limit));
  tile('#t-access', accessTile(d.access));
  tile('#t-turns', turnsTile(d.prohibited_transitions));

  // position
  const at = $('#at');
  if (pos) {
    at.disabled = false;
    if (document.activeElement !== at) at.value = pos.fraction;
    const parts = [
      `<span>${fmt(pos.metres_from_start, 1)} m of ${fmt(seg.length_m, 1)} m</span>`,
      `<span>fraction ${fmt(pos.fraction, 4)}</span>`,
    ];
    if (pos.snapped_from_m != null) parts.push(`<span>snapped ${fmt(pos.snapped_from_m, 1)} m</span>`);
    if (pos.planar_error_m != null) parts.push(`<span class="${pos.planar_error_m > 2 ? 'warn' : ''}">planar would say ${fmt(pos.planar_fraction, 4)} (${fmt(pos.planar_error_m, 1)} m off)</span>`);
    $('#pos-meta').innerHTML = parts.join('');
  } else {
    at.disabled = true;
    $('#pos-meta').innerHTML = '<span>whole segment — pick a position to evaluate `between` rules</span>';
  }

  // rules
  const rules = $('#rules');
  rules.innerHTML = '';
  rules.append(propBlock('Speed limit', d.speed_limit));
  rules.append(propBlock('Access', d.access));
  rules.append(transBlock(d.prohibited_transitions, d.turns));
}

function tile(sel, { value, unit, why, cls }) {
  const t = $(sel);
  t.className = 'tile ' + cls + (String(value).length > 5 ? ' long' : '');
  t.querySelector('.v').innerHTML = esc(value) + (unit ? `<small>${esc(unit)}</small>` : '');
  t.querySelector('.w').textContent = why;
}

function speedTile(p) {
  const extra = p.could_override.length ? ` · could change: #${p.could_override.join(', #')}` : '';
  const cls = p.could_override.length ? 'unk' : 'ok';
  if (!p.value) return { value: 'none', unit: '', why: p.rules.length ? 'no rule matches' : 'none in data', cls: 'none' };
  const max = p.value.max_speed;
  if (max) return { value: String(max.value), unit: max.unit, why: `rule #${p.determining} of ${p.rules.length}${extra}`, cls };
  return { value: p.value.display, unit: '', why: `rule #${p.determining}${extra}`, cls };
}

function accessTile(p) {
  const extra = p.could_override.length ? ` · could change: #${p.could_override.join(', #')}` : '';
  if (!p.value) {
    const why = p.could_override.length ? `depends on #${p.could_override.join(', #')}` : (p.rules.length ? 'no match → class default' : 'none in data');
    return { value: p.rules.length ? 'implied' : 'open', unit: '', why, cls: p.could_override.length ? 'unk' : 'none' };
  }
  const cls = p.value === 'denied' ? 'deny' : (p.could_override.length ? 'unk' : 'ok');
  return { value: p.value, unit: '', why: `rule #${p.determining} of ${p.rules.length}${extra}`, cls };
}

function turnsTile(t) {
  const n = t.in_force.length, u = t.could_apply.length;
  if (!t.rules.length) return { value: '0', unit: '', why: 'none in data', cls: 'none' };
  return { value: String(n), unit: `of ${t.rules.length}`, why: u ? `${u} more depend on missing facts` : 'for your heading', cls: n ? 'deny' : 'ok' };
}

function propBlock(title, p) {
  const box = el('div', 'prop');
  box.append(el('div', 'ph', `<span>${esc(title)} · ${p.rules.length} rule${p.rules.length === 1 ? '' : 's'}</span><span class="res">${esc(shortValue(p.value))}</span>`));
  if (!p.rules.length) { box.append(el('div', 'empty', 'none in data')); return box; }
  for (const r of p.rules) box.append(ruleRow(r, r.index === p.determining ? 'winner' : ''));
  if (p.could_override.length) box.append(el('div', 'note', `? rule${p.could_override.length > 1 ? 's' : ''} #${p.could_override.join(', #')} would override if the missing facts hold`));
  return box;
}

function transBlock(t, turns) {
  const box = el('div', 'prop');
  box.append(el('div', 'ph', `<span>Banned turns · ${t.rules.length} rule${t.rules.length === 1 ? '' : 's'}</span><span class="res">${t.in_force.length} in force</span>`));
  if (!t.rules.length) { box.append(el('div', 'empty', 'none in data')); return box; }
  const inForce = new Set(t.in_force);
  for (const r of t.rules) box.append(ruleRow(r, inForce.has(r.index) ? 'inforce' : ''));
  return box;
}

function ruleRow(r, extra) {
  const marks = { match: '✔', no_match: '✘', undetermined: '?' };
  const row = el('div', `rule ${r.verdict} ${extra}`);
  row.append(el('div', 'mark', marks[r.verdict]));
  const body = el('div');
  body.append(el('div', 'head', `<span class="idx">#${r.index}</span><span>${esc(r.rule)}</span>`));
  if (r.scopes.length) {
    const sc = el('div', 'scopes');
    for (const s of r.scopes) {
      sc.append(el('span', `s ${s.verdict}`, `${marks[s.verdict]} ${esc(s.scope)}`));
      sc.append(el('span', 'b', esc(s.because)));
    }
    body.append(sc);
  }
  row.append(body);
  return row;
}

function shortValue(v) {
  if (v == null) return 'default';
  return typeof v === 'string' ? v : v.display;
}

/* ---------- words ---------- */
function compassShort(deg) {
  return ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'][Math.round(((deg % 360) + 360) % 360 / 45) % 8];
}

function compass(deg) {
  const names = ['north', 'north-east', 'east', 'south-east', 'south', 'south-west', 'west', 'north-west'];
  return names[Math.round(((deg % 360) + 360) % 360 / 45) % 8] + 'bound';
}

function question(d) {
  const seg = d.segment, pos = d.position, s = d.situation;
  const mode = s.mode || 'traveller';
  const verb = mode === 'foot' ? 'walking' : mode === 'bicycle' ? 'cycling' : mode === 'bus' ? 'driving a bus'
    : mode === 'hgv' ? 'driving a heavy goods vehicle' : mode === 'hov' ? 'driving a high-occupancy vehicle'
    : mode === 'emergency' ? 'driving an emergency vehicle' : mode === 'motorcycle' ? 'riding a motorcycle'
    : `driving a ${mode.replace('_', ' ')}`;
  const dir = pos ? compass(state.heading === 'forward' ? pos.bearing_forward_deg : pos.bearing_forward_deg + 180) : state.heading;
  const road = seg.name ? `<b>${esc(seg.name)}</b>` : `<b>an unnamed ${esc(seg.class.replace('_', ' '))}</b>`;
  const routes = seg.routes.filter(r => r !== seg.name);
  const route = routes.length ? ` (${esc(routes.join(', '))})` : '';
  const where = pos ? `, ${fmt(pos.metres_from_start, 0)} m into a ${fmt(seg.length_m, 0)} m stretch` : '';
  const dims = Object.entries(s.vehicle).map(([k, v]) => `${k} ${v}${k === 'weight' ? ' t' : k === 'axle_count' ? '' : ' m'}`);
  const claims = [];
  if (dims.length) claims.push(dims.join(', '));
  if (s.recognized.length) claims.push(`recognised ${s.recognized.join(', ')}`);
  if (s.using.length) claims.push(`here ${s.using.join(', ')}`);
  const when = s.time ? `, at ${new Date(s.time).toLocaleString(undefined, { weekday: 'short', hour: '2-digit', minute: '2-digit' })}` : '';
  return `I\u2019m ${verb} ${dir} on ${road}${route}${where}${when}${claims.length ? ' — ' + esc(claims.join('; ')) : ''}. What applies here?`;
}

initMap();
initControls();
