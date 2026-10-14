# Call it `overrules`, then add "for Overture Maps"

**Recommendation: rename `gers-lr` to `overrules`, with the tagline "overrules — a travel-rules evaluator for Overture Maps".** It is a two-way pun: "Over(ture) rules", and "a later rule overrules an earlier one". The second meaning is exactly the spec's evaluation model: "if more than one rule matches, the last matching rule in the list determines the value" ([Overture docs](https://docs.overturemaps.org/guides/transportation/scoping-and-travel-modes/)). It works for every mode of travel, is short to type as a CLI (`overrules resolve --mode foot --at 0.4`), and was **free on crates.io, npm and PyPI on 14 Oct 2026** (re-checked for this report). The runner-up is **`waywarden`**, the old English parish officer who kept the highways ([Barwick-in-Elmet HS](http://www.barwickinelmethistoricalsociety.com/6353.html)). It is just as free and more distinctive, but has no link to Overture. Keep the trademark "Overture" out of the package name and put it in the tagline. Linux Foundation (LF) policy explicitly allows "`<your product> for <mark>`" and discourages "`<mark> <your product>`" and hyphenated marks ([LF Trademark Usage](https://www.linuxfoundation.org/legal/trademark-usage)). If OMF later adopts the tool, it can gain a prefix, the way Bifrost ships as `overture-bifrost` ([Cargo.toml](https://raw.githubusercontent.com/OvertureMaps/transportation-routing/main/omf-bifrost/overture-bifrost/Cargo.toml)).

## Eight names survive the mode, meaning and registry filters

Ranked by fit to the tool, how mode-neutral the name is, how unique it is, and how it reads as a CLI. ✅ = free, ❌ = taken (crates.io / npm / PyPI, checked 14 Oct 2026).

| # | Name | What it means → how it maps to the tool | C | N | P | Pros | Cons |
|---|---|---|---|---|---|---|---|
| 1 | **overrules** | Over(ture) + rules; later rule overrules earlier = last-match-wins | ✅ | ✅ | ✅ | Names the core semantic. Nods to Overture without using the mark. Mode-neutral. 1 GitHub repo (1★) | Could read as "overrides Overture data". The singular `overrule` is taken on npm/PyPI, and overrule.dev is an AI "policy engine with decision log" ([overrule.dev](https://overrule.dev/)), so search results will be noisy |
| 2 | **waywarden** | Parish "Surveyor of Highways" under a 1691 Act ([Wiktionary](https://en.wiktionary.org/wiki/waywarden)) | ✅ | ✅ | ✅ | Unique (1 GitHub repo, 0★). "Way" covers foot, bike and car. Sounds like a referee | Obscure. "Warden" sounds like enforcement, not explanation. No Overture link |
| 3 | **waywiser** | Surveyor's measuring wheel, from German *Wegweiser*, "shows the way" ([World Wide Words](https://www.worldwidewords.org/ww-way2.html)) | ✅ | ✅ | ✅ | Best story: it measures along the way (linear referencing) and shows the way (rules). "Wise" hints at "explains why" | Clashes with ropensci/waywiser, a geospatial R package with the same GIS audience ([GitHub](https://github.com/ropensci/waywiser)) |
| 4 | **lexpost** | Short for *lex posterior derogat priori*, "the later law repeals the earlier" ([Wikipedia](https://en.wikipedia.org/wiki/Lex_posterior_derogat_priori)) | ✅ | ✅ | ✅ | Literally the evaluation rule. Pairs well with an `explain` command | Opaque. Sounds like a mail or legal-tech brand. The rival maxim *lex specialis* (the more specific rule wins) can confuse people |
| 5 | **wayrule** | "The rule for this way" | ✅ | ✅ | ✅ | Plain, self-explanatory, mode-neutral | Bland and forgettable. Close to WayRuler |
| 6 | **segno** | Musical sign marking a *position* in the score; echoes "seg(ment)" ([Wikipedia](https://en.wikipedia.org/wiki/Dal_segno)) | ✅ | ✅ | ❌ | Elegant link to `at`/`between`. Continues the "Overture" music theme | Pronunciation is unclear (SEN-yo). Says nothing about rules. PyPI is held by a popular QR library |
| 7 | **overture-travel-rules** | Descriptive | ✅ | ✅ | ✅ | Instantly clear. Mode-neutral | Uses the mark as a hyphenated prefix (see next section). Reads as an official OMF ruleset. Long CLI name |
| 8 | **giraldillo** | Seville's rotating weathervane, which shows heading ([Wikipedia](https://en.wikipedia.org/wiki/Giralda)) | ✅ | ✅ | ✅ | Memorable. Fits a Seville demo and the `heading` concept | Hard for English speakers (hee-ral-DEE-yo). Local. Better as the explorer's name than the library's |

**Why `overrules` beats `waywarden`:** it carries the Overture link and the spec's defining rule in one common English word, and a newcomer can guess what it does. `waywarden` wins only if you want zero search noise and no Overture association at all.

## Using "Overture" in the name signals endorsement you don't have

| Fact | Implication |
|---|---|
| LF policy: a mark "should not be used as part of your product name" and "should not be … hyphenated, abbreviated or displayed in parts" ([LF](https://www.linuxfoundation.org/legal/trademark-usage)) | `overture-lr`, `overture-road-rules` and `overture-travel-rules` all break the letter of the policy |
| Allowed patterns: "`<product> for <mark>`", "`<product> compatible with <mark>`" ([LF](https://www.linuxfoundation.org/legal/trademark-usage)) | "overrules — a travel-rules evaluator for Overture Maps" is the textbook form |
| "Overture Maps Foundation™" is *pending or in use*, not registered. "Overture" alone and "GERS" are not on the list ([LF trademarks](https://www.linuxfoundation.org/legal/trademarks)) | The legal risk is low today. The bigger risk is **implied endorsement** |
| OMF's own crates claim the `overture-*` prefix (`overture-stac`, `overture-bifrost`, `overture-types`) ([crates.io](https://crates.io/api/v1/crates?q=overture)) | A third-party `overture-lr` would sit next to official crates and look official |
| The policy forbids claiming certification "unless … explicit authorization" ([LF](https://www.linuxfoundation.org/legal/trademark-usage)) | Keep "reference", "conformance" and "canonical" out of the name until OMF signs off |

What is still unknown: I found no OMF-specific brand guide and no case of LF asking anyone to rename over "Overture". Many community `overture*` packages already exist, such as `maplibre-gl-overture-maps` and `overture2osrm`. Ask trademarks@linuxfoundation.org or the assignees of [OvertureMaps/data#469](https://github.com/OvertureMaps/data/issues/469), which asks for exactly this reference rule evaluator and viewer.

Does the "Over-" pun count as "displaying the mark in parts"? Strictly, perhaps. In practice no: "overrule" is an ordinary English word, and it does not claim to be Overture's product. That is the honest grey area of option 1. If it worries you, `waywarden` avoids it entirely.

**Adoption path:**
- **Today:** a brand name plus "for Overture Maps", in your own org.
- **If OMF adopts it:** transfer the repo to OvertureMaps and optionally add the prefix, as `omf-bifrost` did with `overture-bifrost` ([README](https://github.com/OvertureMaps/transportation-routing/tree/main/omf-bifrost)).
- **Caveat:** `overture-overrules` is redundant. If adopted, OMF would more likely keep `overrules` or publish `overture-rules`, which is free on all three registries. `waywarden` → `overture-waywarden` prefixes more cleanly.

## Names to avoid, and why

| Name | Problem |
|---|---|
| `overture-road-rules` | **INRIX sells "Road Rules"**, a curb and travel-lane rules platform in this exact domain ([INRIX](https://inrix.com/products/road-rules/)). "Road" also undersells pedestrians and cyclists |
| `gers-lr` | GERS (Global Entity Reference System) is about IDs, not rules. "lr" is ambiguous (learning rate, LR parsers). "gers" has an unclear hard or soft g |
| `overture-lr` | Same "lr" ambiguity. Uses the mark as a prefix |
| `overturn` | In transport it means a vehicle rollover. It also covers turns only |
| `rightofway`, `highwaycode` | "Right of way" is a crowded land-acquisition industry term ([irth](https://irthsolutions.com/stakeholder-and-land-management)). "Highway Code" is a UK government publication |
| `wayfarer`, `waymark`, `waypoint` | Niantic/Scopely Wayfarer is a POI tool ([Scopely](https://wayfarer.scopely.com/)). Waymark is a WordPress mapping plugin ([GitHub](https://github.com/OpenGIS/Waymark)). Waypoint is a HashiCorp product |
| `hecate`, `janus`, `hermes`, `terminus` | Hecate is a Mapbox geospatial feature store ([GitHub search](https://api.github.com/search/repositories?q=hecate+in:name)). Janus is a WebRTC gateway, Hermes is Meta's JS engine, and Terminus is TerminusDB; all are taken or crowded |
| `mojon`, `vorfahrt`, `cañada`→`canada` | Spanish slang for "turd"; sounds rude in English; reads as the country ([RAE](https://dle.rae.es/moj%C3%B3n)) |
| `tempo`, `coda`, `segue` | Taken, and each covers only one part of the scope: speed, last-wins or transitions |
| Legba, Eshu, Xolotl | Figures in living religions. `legba` is also a credential brute-forcer crate |
| Anything with `-rs`, `reference`, `canonical` | The Rust API Guidelines say no `-rs` ([guidelines](https://rust-lang.github.io/api-guidelines/naming.html)). Authority words imply endorsement you don't have |

## One brand across library, CLI, explorer and test vectors

Use one brand for the code and a plain descriptive name for the conformance data. That is how authoritative suites are named: "JSON Schema Test Suite", "Test262" and "Canonical GTFS Schedule Validator" all get their authority from the spec owner, not from a clever name ([JSON-Schema-Test-Suite](https://github.com/json-schema-org/JSON-Schema-Test-Suite), [test262](https://github.com/tc39/test262), [gtfs-validator](https://github.com/MobilityData/gtfs-validator)).

| Piece | Name | Notes |
|---|---|---|
| Rust library | `overrules` (crate) | One word avoids the hyphen/underscore mismatch: `use overrules::…` |
| CLI | binary `overrules` (from crate `overrules-cli`, free) | `overrules resolve`, `overrules explain`, `overrules locate` |
| WASM / JS | npm `overrules` (or `overrules-wasm`, free) | The same pattern as PMTiles (`pmtiles` on npm and PyPI) ([PMTiles](https://github.com/protomaps/PMTiles)) |
| Python (later) | PyPI `overrules` (free) | Reserve it early |
| Web explorer | "Overrules Explorer", or a sub-brand like **giraldillo** for the Seville demo | Optional flavour: call position pins "segno" and the `explain` output the "libretto" |
| Conformance vectors | `transportation-rules-test-vectors` (descriptive, no brand) | Designed to move into OvertureMaps later. Say "conformance" only once OMF endorses it |
| Tagline | "overrules — a travel-rules evaluator for Overture Maps: speed limits, access and turn restrictions for every traveller, with the rule that decided it" | Follows the LF-approved "for <mark>" form |

**Next steps:**
1. Reserve `overrules` and `overrules-cli` on crates.io, and `overrules` on npm and PyPI.
2. Check the .dev/.org domains. I did not check domains or run USPTO/EUIPO trademark searches.
3. Ask the #469 assignees whether OMF would rather host it.

## Conclusion

The naming question really decides how the tool relates to OMF. A brand that does not contain the mark keeps the project portable and honest about who stands behind it. A descriptive `overture-*` name only makes sense once OMF says yes. `overrules` sits in the useful middle: it hints at Overture, states the spec's last-match-wins rule, and costs nothing to rename later. Split the conformance vectors out under a plain, spec-tied name from day one. That artefact is the one most likely to become official, and it should not inherit a pun.
