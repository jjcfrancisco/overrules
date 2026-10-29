# Semantics

How `overrules` reads Overture's transportation rules, and every place where the spec leaves a
choice open. Each choice below is deliberate: it is tested, and it changes only with a changelog
entry.

Sources:

- **Guide**: [Scoping and travel modes](https://docs.overturemaps.org/guides/transportation/scoping-and-travel-modes/) in the Overture docs.
- **Schema 2.0.0**: [`schema/transportation/segment.yaml`](https://github.com/OvertureMaps/schema/blob/v2.0.0/schema/transportation/segment.yaml) and [`schema/defs.yaml`](https://github.com/OvertureMaps/schema/blob/v2.0.0/schema/defs.yaml), and the Pydantic models in [`packages/`](https://github.com/OvertureMaps/schema/tree/v2.0.0/packages).

## 0. Schema version

`overrules` reads data written to one Overture schema version, `overrules::SCHEMA_VERSION`
(currently **2.0.0**, used by releases `2026-09-23.0` and `2026-09-23.1`). Its lists of values (headings,
travel modes, vehicle dimensions, comparisons, units, access types) are exactly that version's.

**Choice:** strict. A value outside those lists (say a new access type) is not guessed at: the
file fails to load with an error naming the segment and the field, e.g.
`segment cae6…: cannot read access_restrictions[2].access_type: unknown variant "permissive"`.
Travel modes are the one exception (§3).

This describes the **supported fields**, not full schema validation. Readers ignore unmodelled
fields. Names, classes, purposes and statuses remain strings. Numeric schema constraints are
not all enforced, and inverted ranges are deliberately accepted (§5). A successful read does
not certify that the input conforms to the complete Overture schema.

`SCHEMA_VERSION` is a declared target, not an input-version check. The readers do not contact
STAC or reject a file solely because it came from another release. Applications that need a
version check must retain the source release and consult its catalogue entry.

Overture announces which schema each release uses in its
[release calendar](https://docs.overturemaps.org/release-calendar.json), ahead of the release,
and in each release's [STAC catalogue](https://stac.overturemaps.org) entry. A weekly CI job
(`scripts/check_schema.py`, `just schema-check`) compares the lists here with the schema of the
latest-dated calendar entry with a non-null schema version. Early detection depends on when
the calendar and schema tag become available and when the job next runs.

The check covers eight enum lists, not field additions, requiredness, numeric bounds, or rule
semantics. Matching lists alone do not establish compatibility with a new schema. See
[the maintenance steps](../CONTRIBUTING.md#when-the-schema-check-fails).

## 1. Resolving a rule list

The guide states this explicitly, and `overrules` implements it as written.

1. A rule **matches** when every one of its scoping conditions holds. Conditions are combined with AND.
2. The **determining rule** is the *last* matching rule in the list.
3. With no matching rule, a **default** applies (see §8).

`prohibited_transitions` is the exception: it lists turns that are banned, so *every* matching
rule is in force, not only the last one.

## 2. Missing facts: three-valued evaluation

The guide assumes every fact is known. In practice a caller often does not know the time, the
vehicle's weight, or even the direction of travel. `overrules` does not guess:

| A condition is… | when… |
|---|---|
| `match` | the fact is given and satisfies it |
| `no_match` | the fact is given and does not satisfy it |
| `undetermined` | the fact it depends on was not given |

A rule's verdict is `no_match` if any condition fails, otherwise `undetermined` if any is unknown,
otherwise `match`. A failure outweighs an unknown: a rule for `heading: forward` cannot apply to
someone travelling backward, whatever the time.

Undetermined rules **never** determine a value. Every undetermined rule that comes *after* the
determining rule (or every one, if none determined) is listed in `could_override`: if its missing
facts held, it would win. An empty `could_override` means the answer is settled.

## 3. Travel modes

The guide shows the taxonomy as a diagram. The schema has one comment
(`motor_vehicle includes car, truck and motorcycle`), and the sources disagree:

| Source | `truck` | `bus` |
|---|---|---|
| Schema comment on `travelMode` | under `motor_vehicle` | not mentioned |
| Guide diagram | not shown | under `motor_vehicle` |
| Maintainer reply, [discussion #301](https://github.com/orgs/OvertureMaps/discussions/301) (2024) | "we currently don't output truck" | "motor_vehicle does not contain bus" |

**Choice:** the union of the schema and the diagram.

```
vehicle
├── bicycle
└── motor_vehicle
    ├── car, truck, motorcycle          (schema comment)
    └── bus, hgv, hov, emergency        (guide diagram)
foot
```

A rule scoped to a group applies to every mode inside it (`car` matches `mode: [motor_vehicle]`).
The other direction is undetermined: a traveller who says only `motor_vehicle` might or might not
be the `car` a rule names, so `overrules` asks for a more specific mode.

Modes the crate does not know are kept when data is read (`TravelMode::Other`) and match only
themselves, so a new mode in a future schema does not stop a file from loading.

## 4. `using` and `recognized`

These are claims the traveller makes: a purpose (`to_deliver`) or a status (`as_private`).
**Choice:** a claim that is not made is not held. A rule scoped to `recognized: [as_private]` does
not match a traveller who declared no status (`no_match`, not `undetermined`). Otherwise every
resident-only exception would leave every answer unsettled.

## 5. `between`

- A position matches a range if `start ≤ position ≤ end`, **ends included**, with a 1e-9
  tolerance for positions that were computed rather than read from the data.
- The JSON Schema bounds each end to `0..=1` but cannot require `start < end` (its own comment
  says so). The newer Pydantic models reject inverted ranges. **Choice:** an inverted range is read
  as the same stretch in order, and the explanation says it was written inverted.
- Positions are fractions of the segment's **geodesic length on WGS84**, as the guide defines
  them. `overrules` never uses planar (lon/lat as x/y) maths to evaluate a rule. The
  [`lr::planar`](../src/lr/planar.rs) module exists only to measure how far off
  planar maths would be.

## 6. Vehicle conditions

- The traveller's dimensions are compared in **metres**, **metric tonnes**, or a plain count
  (`axle_count`). A rule's threshold is converted to those units first.
- The schema's `st` is the **short ton** (2,000 lb) and `lt` the **long ton** (2,240 lb).
  Neither is the stone. The Pydantic model's description of `lt` says "2,400 pounds". The
  long ton is 2,240 lb, and that is what `overrules` uses.
- A threshold with no unit is read as metres or tonnes.
- A unit that cannot measure the dimension (feet for a weight) makes the condition
  `undetermined`, with the reason, rather than silently wrong.
- Values within 1e-9 of each other count as equal for **every** operator, so `>` and `<` are
  strict by more than rounding: a rule `height < 12 ft` (3.6576000000000004 m) does not match
  a vehicle given as 3.6576 m.

## 7. `heading` and `during`

- `forward` means travelling in the order of the segment's coordinates.
- `during` uses the OpenStreetMap `opening_hours` syntax, evaluated with the
  [`opening-hours`](https://crates.io/crates/opening-hours) crate against the traveller's
  **local** time (no time zone).
- Some Overture values are missing separators that `opening_hours` requires, e.g.
  `Mo-Fr07:00-18:00` for `Mo-Fr 07:00-18:00`. In the `2026-09-23.1` New York City extract, 217
  of 609 time rules are written this way. **Choice:** if a value does not parse as written,
  `overrules` restores three missing separators and tries once more: the space between days and
  times, the comma between back-to-back time ranges (`09:0014:00`), and the comma between
  back-to-back days (`SaSu`). The explanation shows the value as read. The data itself is not
  changed.
- An expression that still does not parse, or that evaluates to `unknown`, leaves the condition
  `undetermined`.
- **Since 0.2:** an expression containing public or school holidays (`PH`, `SH`) or solar
  events (`sunrise`, `sunset`, `dawn`, `dusk`, including offsets) is also `undetermined`.
  This API does not supply the holiday calendar, location and time zone those expressions
  require. It never substitutes an empty calendar or fixed solar times. Detection uses the
  parsed expression, so these words inside quoted comments do not affect evaluation.
  The whole time expression is conservatively undetermined, including holiday exceptions.
  Another failed scope, such as an opposite heading, still makes the rule `no_match`.
- Dates outside the evaluator's supported years, 1900 through 9999, are `undetermined`.
- `PreparedSegment` caches distinct time expressions, including repairs and parse errors.
  Its compact and detailed methods use the same condition checks. Compact answers omit
  explanations but retain all potentially overriding or applicable rules.

## Geometry queries (since 0.2)

- Segment vertices and query points must have finite WGS84 longitude in `-180..=180` and
  latitude in `-90..=90`. Invalid queries return errors from `project`, `nearest` and `snap`.
- Nearest search uses Earth-centred boxes enclosing each entire geodesic arc. Distance to
  a box is a lower bound on chord distance, which is a lower bound on geodesic distance.
  Candidates are examined until none can improve the requested results. There is no fixed
  candidate cutoff, longitude wrapping issue or mean-latitude approximation.
- Projection converges to a bracket of 1e-11 of each arc, within the 1e-9 fraction tolerance
  used by `between`. As with all coordinate calculations, floating-point precision limits
  accuracy on extremely short lines.
- Bearings describe the local forward direction at the supplied position. Duplicate
  vertices are skipped, using the following nonzero arc when available and the preceding
  one at the end. Entirely zero length lines have bearing zero.
- A slice with identical endpoints retains two equal coordinates, allowing it to be used
  as a LineString or passed to `Segment::new`.

## 8. Defaults

| Property | With no matching rule |
|---|---|
| `access_restrictions` | `Access::Default`. Overture says "the default restrictions for the road class govern" but publishes no table of them ([discussion #307](https://github.com/orgs/OvertureMaps/discussions/307)). `overrules` does not invent one: a footway with no rules is not therefore open to cars. |
| `speed_limits` | No limit from the data. Fall back to the jurisdiction's default for the class. |
| `prohibited_transitions` | No explicit prohibition is known to match. Undetermined rules remain in `could_apply`. Access restrictions and network connectivity must still be considered by the application. |

## Not yet covered

- Whether a `prohibited_transitions` sequence is connected (each step's connector really joins
  the previous segment). This needs the whole network, not one segment.
- Rules that can never win because a later unconditional rule always overrides them.
- `road_surface`, `road_flags`, `level_rules`, `subclass_rules` and `width_rules`, which are
  scoped by `between` only and are not read yet.
