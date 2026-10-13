# gers-lr — Overture linear referencing + rule resolver (Seville demo)

A small Rust library/CLI that answers one question correctly:

> *Standing at **this point** on **this Overture segment**, travelling **this way**, as **this kind of traveller**, **right now** — what rules apply, and why?*

Overture's transportation theme attaches `speed_limits`, `access_restrictions` and
`prohibited_transitions` to segments as **lists of scoped rules**. Each rule may be limited
by `when` (heading, time, travel mode, purpose, status, vehicle dimensions) and by `between`
(a geodesic fraction range along the segment). The resolution semantics are:

* all scopes of a rule must hold (**AND**);
* the **last** matching rule in the list wins;
* otherwise the property default applies;
* `between` fractions are **geodesic on WGS84**, not planar.

Most consumers either read `speed_limits[0]` or project with planar maths. This tool shows
what that costs, on real data, for a real city.

## Data (6.8 MB, not 400 GB)

Seville, Spain, release `2026-09-23.1`, bbox lon −6.05..−5.85 / lat 37.30..37.46.

```sh
duckdb < data/extract_seville.sql      # ~2 min; writes data/seville/{segment,connector}.parquet
```

43,749 segments · 54,896 connectors.

## Build

```sh
cargo build --release
cargo test --release                   # 5 tests: geodesic LR + rule semantics
```

## Demo

### 1 · Same spot, opposite directions → different speed limits

Segment `e26d3384…` (secondary road, 996 m) carries **8** speed-limit rules scoped by both
heading and `between`. Rule #0 says 90 km/h — that's what a naive reader sees.

```sh
gers-lr resolve --segment e26d3384 --at 0.05 --heading forward  --mode car   # ⇒ max 80 km/h  [rule #5]
gers-lr resolve --segment e26d3384 --at 0.05 --heading backward --mode car   # ⇒ max 50 km/h  [rule #4]
```

The report shows every rule, every scope, ✔/✘/?, and names the determining rule.

### 2 · Living street with exceptions (heading + mode + purpose + status)

`Calle Pintor Rosales`, 4 access rules: denied backward; allowed on foot `at_destination`;
allowed for bicycles/motor vehicles `as_private`; allowed `as_private`.

```sh
gers-lr resolve --segment 1b946971 --heading backward --mode car                         # ⇒ DENIED   [rule #0]
gers-lr resolve --segment 1b946971 --heading backward --mode car --recognized as_private # ⇒ ALLOWED  [rule #3]
gers-lr resolve --segment 1b946971 --heading backward --mode foot --using at_destination # ⇒ ALLOWED  [rule #1]
```

Note `car ⊂ motor_vehicle`: the mode taxonomy is applied, not string equality.

### 3 · Vehicle dimensions and *undetermined* facts

`Avenida del Almendral`: denied when `weight > 3.5 t`.

```sh
gers-lr resolve --segment 645f3ec7 --heading forward --mode truck --vehicle weight=3.5  # ⇒ IMPLIED (3.5 is not > 3.5)
gers-lr resolve --segment 645f3ec7 --heading forward --mode truck --vehicle weight=12   # ⇒ DENIED  [rule #1]
gers-lr resolve --segment 645f3ec7 --heading forward --mode truck                       # ⇒ IMPLIED, but
#   ? rule(s) [1] come later and are undetermined — they would override if their missing facts hold
```

Evaluation is **three-valued**: a scope whose fact you did not supply is *undetermined*, and
the verdict tells you which later rules could still flip it. No silent guessing.

### 4 · Prohibited transitions

`Paseo de las Delicias` (`a022afe5…`) has 17 of them; each is resolved against your heading
and printed with the via-connector and the target segment's name/class.

### 5 · Geodesic linear referencing

```sh
gers-lr locate --lon -5.9926 --lat 37.3861        # the Giralda
```

```
  gers id       segment                                       offset   geodesic     planar   Δ planar
  2e61507f-7b23 "Puerta de Palos" (road/footway)               7.4 m     1.0000     1.0000     0.00 m
  3bbf96ee-ff34 "Plaza Virgen de los Reyes" (road/footway)    24.7 m     0.1402     0.1206     2.54 m
  563b148c-1c9b "Calle Alemanes" (road/living_street)         28.0 m     0.6408     0.6197     3.07 m
  ae3f22e4-7fef <unnamed> (road/unknown)                      41.2 m     0.2043     0.0399     9.13 m
```

`Δ planar` = if you stored the planar fraction and someone read it back per Overture's
geodesic definition, this is how far along the road they would land from the true point.

```sh
gers-lr at --segment e26d3384 --fraction 0.123562054   # the 90→70 boundary: geodesic vs planar point, 1.98 m apart
gers-lr resolve --lon -6.0030 --lat 37.3866 --heading forward --mode car   # snap, then resolve at the snapped fraction
gers-lr show --segment 645f3ec7                         # GeoJSON: geometry + connectors at their fractions
```

### 6 · City-wide audit: what planar maths would cost Seville

```sh
gers-lr audit
```

```
  segments with between-scoped rules : 644
  boundary fractions checked         : 1832
  mean displacement                  : 1.97 m
  histogram                          : <1 m 1317  · 1–5 m 297  · 5–10 m 111  · 10–25 m 96  · ≥25 m 11
  segments with a boundary > 5 m off : 117

  worst offenders:
  6383437f-209f <unnamed> (road/track)                         3612 m       2    54.58 m    0.1456
  55c99d8b-d69e "SE-40" (road/motorway)                        3787 m       2    50.38 m    0.6562
  ee215add-60fd "SE-40" (road/motorway)                        3707 m       4    46.03 m    0.3482
```

18 % of Seville's `between`-scoped segments would have a rule boundary misplaced by more than
5 m; a speed-limit change on the SE-40 motorway would move by 50 m.

## What Seville doesn't have

Zero `during` (time-scoped) rules in this extract. The evaluator supports them
(OpenStreetMap `opening_hours` syntax via the `opening-hours` crate, `--time 2026-10-13T08:30`
or `--time now`) — covered by a unit test, but there is nothing in this city to point it at.

## Layout

| file | what |
|---|---|
| `src/lr.rs` | `Measure` (cumulative geodesic metres), `point_at` (fraction → point), `project` (point → fraction; golden-section on geodesic distance with triangle-inequality pruning) |
| `src/rules.rs` | `Situation`, three-valued `Verdict`, per-scope checks, mode taxonomy, last-match-wins resolution with provenance |
| `src/model.rs` | serde model of Overture segment rules |
| `src/reader.rs` | Parquet → `Segment` (WKB via geozero) |
| `src/index.rs` | `Network`: id/name lookup, R-tree, nearest-segment with exact geodesic projection |
| `src/main.rs` | CLI: `stats · find · locate · at · resolve · show · audit` |

## Not in scope (yet)

OpenLR encode/decode, cross-release GERS id stability, connector-graph routing.
