# Semantics

How `overrules` reads Overture's transportation rules, and every place where the spec leaves a
choice open. Each choice below is deliberate: it is tested, and it changes only with a changelog
entry.

Sources:

- **Guide**: [Scoping and travel modes](https://docs.overturemaps.org/guides/transportation/scoping-and-travel-modes/) in the Overture docs.
- **Schema**: [`schema/transportation/segment.yaml`](https://github.com/OvertureMaps/schema/blob/main/schema/transportation/segment.yaml) and [`schema/defs.yaml`](https://github.com/OvertureMaps/schema/blob/main/schema/defs.yaml).

## 1. Resolving a rule list

The guide states this explicitly; `overrules` implements it as written.

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
  them. `overrules` never uses planar (lon/lat as x/y) maths to evaluate a rule; the
  [`lr::planar`](../src/lr/planar.rs) module exists only to measure how far off
  planar maths would be.

## 6. Vehicle conditions

- The traveller's dimensions are compared in **metres**, **metric tonnes**, or a plain count
  (`axle_count`). A rule's threshold is converted to those units first.
- The schema's `st` is the **short ton** (2,000 lb) and `lt` the long ton (2,240 lb), per the
  comments on `weightUnit`. Neither is the stone.
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
  **local** time (no time zone). An expression that does not parse, or that evaluates to
  `unknown`, leaves the condition `undetermined`.

## 8. Defaults

| Property | With no matching rule |
|---|---|
| `access_restrictions` | `Access::Default`. Overture says "the default restrictions for the road class govern" but publishes no table of them ([discussion #307](https://github.com/orgs/OvertureMaps/discussions/307)). `overrules` does not invent one: a footway with no rules is not therefore open to cars. |
| `speed_limits` | No limit from the data. Fall back to the jurisdiction's default for the class. |
| `prohibited_transitions` | Every transition is allowed ("permissive by default", per the guide). |

## Not yet covered

- Whether a `prohibited_transitions` sequence is connected (each step's connector really joins
  the previous segment). This needs the whole network, not one segment.
- Rules that can never win because a later unconditional rule always overrides them.
- `road_surface`, `road_flags`, `level_rules`, `subclass_rules` and `width_rules`, which are
  scoped by `between` only and are not read yet.
