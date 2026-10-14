# overrules

Evaluate [Overture Maps](https://overturemaps.org) transportation rules (speed limits,
access restrictions, prohibited turns) for any traveller, on foot, by bike or at the wheel, and
say which rule decided and why.

```rust
use overrules::{io, Heading, Network, Situation, TravelMode};

let net = Network::new(io::read_path("seville.geojson")?);
let segment = net.lookup("e26d3384")?;
let answer = overrules::resolve(segment, &Situation::new().at(0.05).heading(Heading::Forward).mode(TravelMode::Car))?;
println!("{:?} decided by rule {:?}", answer.speed_limit_rule(), answer.speed_limit.determining);
# Ok::<(), overrules::Error>(())
```

Features: `parquet` (read Overture GeoParquet), `pack` (compact snapshots for the browser).

See the [repository](https://github.com/jjcfrancisco/overrules) for the CLI, the web explorer,
and the semantics this crate implements.
