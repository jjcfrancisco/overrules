use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Direction of travel along a segment, relative to the order of its coordinates.
///
/// `forward` runs from the first coordinate to the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Heading {
    /// From the first coordinate towards the last.
    Forward,
    /// From the last coordinate towards the first.
    Backward,
}

impl Heading {
    /// Both headings.
    pub const ALL: [Self; 2] = [Self::Forward, Self::Backward];

    /// The other direction.
    #[must_use]
    pub fn opposite(self) -> Self {
        match self {
            Self::Forward => Self::Backward,
            Self::Backward => Self::Forward,
        }
    }

    /// The spelling Overture uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Backward => "backward",
        }
    }
}

impl fmt::Display for Heading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

impl FromStr for Heading {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "forward" => Ok(Self::Forward),
            "backward" => Ok(Self::Backward),
            _ => Err(Error::invalid("heading", s, "expected forward or backward")),
        }
    }
}

/// A travel mode from Overture's taxonomy.
///
/// Some modes are groups of others: [`TravelMode::lineage`] gives each mode with its
/// ancestors, and a rule scoped to a group applies to every mode inside it. Where the
/// sources disagree on the tree, the choice made here is documented in
/// `docs/semantics.md`.
///
/// Parsing a string with [`FromStr`] rejects unknown modes (it is meant for user input);
/// deserialising data keeps them as [`TravelMode::Other`], so a schema that grows a new
/// mode does not stop a whole file from loading.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum TravelMode {
    /// Any vehicle, motorised or not.
    Vehicle,
    /// Any motor vehicle.
    MotorVehicle,
    /// A car.
    Car,
    /// A truck.
    Truck,
    /// A motorcycle.
    Motorcycle,
    /// On foot.
    Foot,
    /// A bicycle.
    Bicycle,
    /// A bus.
    Bus,
    /// A heavy goods vehicle.
    Hgv,
    /// A high-occupancy vehicle.
    Hov,
    /// An emergency vehicle.
    Emergency,
    /// A mode this version of the crate does not know. It matches only itself.
    Other(String),
}

impl TravelMode {
    /// Every mode Overture defines, in schema order.
    pub const ALL: [Self; 11] = [
        Self::Vehicle,
        Self::MotorVehicle,
        Self::Car,
        Self::Truck,
        Self::Motorcycle,
        Self::Foot,
        Self::Bicycle,
        Self::Bus,
        Self::Hgv,
        Self::Hov,
        Self::Emergency,
    ];

    /// The spelling Overture uses.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Vehicle => "vehicle",
            Self::MotorVehicle => "motor_vehicle",
            Self::Car => "car",
            Self::Truck => "truck",
            Self::Motorcycle => "motorcycle",
            Self::Foot => "foot",
            Self::Bicycle => "bicycle",
            Self::Bus => "bus",
            Self::Hgv => "hgv",
            Self::Hov => "hov",
            Self::Emergency => "emergency",
            Self::Other(s) => s,
        }
    }

    /// The group this mode belongs to, if any.
    pub fn parent(&self) -> Option<Self> {
        match self {
            Self::Vehicle | Self::Foot | Self::Other(_) => None,
            Self::Bicycle | Self::MotorVehicle => Some(Self::Vehicle),
            Self::Car
            | Self::Truck
            | Self::Motorcycle
            | Self::Bus
            | Self::Hgv
            | Self::Hov
            | Self::Emergency => Some(Self::MotorVehicle),
        }
    }

    /// This mode followed by each of its ancestors, narrowest first.
    ///
    /// ```
    /// use overrules::TravelMode;
    /// let names: Vec<_> = TravelMode::Car.lineage().iter().map(|m| m.as_str().to_owned()).collect();
    /// assert_eq!(names, ["car", "motor_vehicle", "vehicle"]);
    /// ```
    pub fn lineage(&self) -> Vec<Self> {
        let mut out = vec![self.clone()];
        while let Some(p) = out.last().and_then(Self::parent) {
            out.push(p);
        }
        out
    }

    /// Whether this mode is `other` or one of its descendants (`car` is within `vehicle`).
    pub fn is_within(&self, other: &Self) -> bool {
        self.lineage().contains(other)
    }
}

impl fmt::Display for TravelMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

impl FromStr for TravelMode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|m| m.as_str() == s)
            .ok_or_else(|| {
                let known: Vec<&str> = Self::ALL.iter().map(Self::as_str).collect();
                Error::invalid(
                    "travel mode",
                    s,
                    format!("expected one of {}", known.join(", ")),
                )
            })
    }
}

impl From<String> for TravelMode {
    fn from(s: String) -> Self {
        s.parse().unwrap_or(Self::Other(s))
    }
}

impl From<TravelMode> for String {
    fn from(m: TravelMode) -> Self {
        match m {
            TravelMode::Other(s) => s,
            known => known.as_str().to_owned(),
        }
    }
}

/// A vehicle dimension a rule can be scoped by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    /// Number of axles.
    AxleCount,
    /// Height, compared in metres.
    Height,
    /// Length, compared in metres.
    Length,
    /// Weight, compared in metric tonnes.
    Weight,
    /// Width, compared in metres.
    Width,
}

impl Dimension {
    /// Every dimension Overture defines.
    pub const ALL: [Self; 5] = [
        Self::AxleCount,
        Self::Height,
        Self::Length,
        Self::Weight,
        Self::Width,
    ];

    /// The spelling Overture uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AxleCount => "axle_count",
            Self::Height => "height",
            Self::Length => "length",
            Self::Weight => "weight",
            Self::Width => "width",
        }
    }

    /// What kind of quantity this dimension measures.
    pub fn quantity(self) -> Quantity {
        match self {
            Self::AxleCount => Quantity::Count,
            Self::Height | Self::Length | Self::Width => Quantity::Length,
            Self::Weight => Quantity::Weight,
        }
    }
}

impl fmt::Display for Dimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

impl FromStr for Dimension {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|d| d.as_str() == s)
            .ok_or_else(|| {
                Error::invalid(
                    "vehicle dimension",
                    s,
                    "expected axle_count, height, length, weight or width",
                )
            })
    }
}

/// What a [`Dimension`] or [`Unit`] measures, and so which canonical unit applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Quantity {
    /// A plain count (axles).
    Count,
    /// A length, canonically in metres.
    Length,
    /// A weight, canonically in metric tonnes.
    Weight,
}

impl Quantity {
    /// The unit values of this quantity are compared in.
    pub fn canonical_unit(self) -> Option<Unit> {
        match self {
            Self::Count => None,
            Self::Length => Some(Unit::M),
            Self::Weight => Some(Unit::T),
        }
    }
}

/// A unit of length or weight from Overture's `lengthUnit`/`weightUnit` enums.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    /// Inch.
    In,
    /// Foot.
    Ft,
    /// Yard.
    Yd,
    /// Mile.
    Mi,
    /// Centimetre.
    Cm,
    /// Metre.
    M,
    /// Kilometre.
    Km,
    /// Ounce.
    Oz,
    /// Pound.
    Lb,
    /// Short ton (2,000 lb). Overture's `st` is the short ton, not the stone.
    St,
    /// Long ton (2,240 lb).
    Lt,
    /// Gram.
    G,
    /// Kilogram.
    Kg,
    /// Metric tonne.
    T,
}

impl Unit {
    /// Every unit Overture defines.
    pub const ALL: [Self; 14] = [
        Self::In,
        Self::Ft,
        Self::Yd,
        Self::Mi,
        Self::Cm,
        Self::M,
        Self::Km,
        Self::Oz,
        Self::Lb,
        Self::St,
        Self::Lt,
        Self::G,
        Self::Kg,
        Self::T,
    ];

    /// The spelling Overture uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::In => "in",
            Self::Ft => "ft",
            Self::Yd => "yd",
            Self::Mi => "mi",
            Self::Cm => "cm",
            Self::M => "m",
            Self::Km => "km",
            Self::Oz => "oz",
            Self::Lb => "lb",
            Self::St => "st",
            Self::Lt => "lt",
            Self::G => "g",
            Self::Kg => "kg",
            Self::T => "t",
        }
    }

    /// Whether this is a unit of length or of weight.
    pub fn quantity(self) -> Quantity {
        match self {
            Self::In | Self::Ft | Self::Yd | Self::Mi | Self::Cm | Self::M | Self::Km => {
                Quantity::Length
            }
            Self::Oz | Self::Lb | Self::St | Self::Lt | Self::G | Self::Kg | Self::T => {
                Quantity::Weight
            }
        }
    }

    /// Converts `value` in this unit to the canonical unit of its quantity
    /// (metres or metric tonnes).
    pub fn to_canonical(self, value: f64) -> f64 {
        let factor = match self {
            Self::In => 0.0254,
            Self::Ft => 0.3048,
            Self::Yd => 0.9144,
            Self::Mi => 1_609.344,
            Self::Cm => 0.01,
            Self::M | Self::T => 1.0,
            Self::Km => 1_000.0,
            Self::Oz => 0.028_349_523_125e-3,
            Self::Lb => 0.453_592_37e-3,
            Self::St => 0.907_184_74,
            Self::Lt => 1.016_046_908_8,
            Self::G => 1e-6,
            Self::Kg => 1e-3,
        };
        value * factor
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

impl FromStr for Unit {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|u| u.as_str() == s)
            .ok_or_else(|| {
                Error::invalid(
                    "unit",
                    s,
                    "expected in, ft, yd, mi, cm, m, km, oz, lb, st, lt, g, kg or t",
                )
            })
    }
}

/// A comparison operator in a vehicle selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    /// `>`
    GreaterThan,
    /// `>=`
    GreaterThanEqual,
    /// `==`
    Equal,
    /// `<`
    LessThan,
    /// `<=`
    LessThanEqual,
}

impl Comparison {
    /// Every comparison the supported schema defines.
    pub const ALL: [Self; 5] = [
        Self::GreaterThan,
        Self::GreaterThanEqual,
        Self::Equal,
        Self::LessThan,
        Self::LessThanEqual,
    ];

    /// The spelling Overture uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GreaterThan => "greater_than",
            Self::GreaterThanEqual => "greater_than_equal",
            Self::Equal => "equal",
            Self::LessThan => "less_than",
            Self::LessThanEqual => "less_than_equal",
        }
    }

    /// Whether `lhs <op> rhs` holds.
    ///
    /// Values within [`Comparison::TOLERANCE`] of each other count as equal for every
    /// operator, so a limit converted between units (12 ft is `3.6576000000000004` m) is not
    /// crossed by rounding alone.
    pub fn holds(self, lhs: f64, rhs: f64) -> bool {
        let equal = (lhs - rhs).abs() <= Self::TOLERANCE;
        match self {
            Self::GreaterThan => lhs > rhs && !equal,
            Self::GreaterThanEqual => lhs > rhs || equal,
            Self::Equal => equal,
            Self::LessThan => lhs < rhs && !equal,
            Self::LessThanEqual => lhs < rhs || equal,
        }
    }

    /// How close, in metres, tonnes or a count, two values must be to count as equal.
    pub const TOLERANCE: f64 = 1e-9;

    /// The operator as a symbol, e.g. `>=`.
    pub fn symbol(self) -> &'static str {
        match self {
            Self::GreaterThan => ">",
            Self::GreaterThanEqual => ">=",
            Self::Equal => "==",
            Self::LessThan => "<",
            Self::LessThanEqual => "<=",
        }
    }
}

/// One condition on the traveller's vehicle, e.g. `weight > 3.5 t`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VehicleSelector {
    /// What is measured.
    pub dimension: Dimension,
    /// How it is compared.
    pub comparison: Comparison,
    /// The threshold, in `unit`.
    pub value: f64,
    /// Unit of `value`. Absent for counts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<Unit>,
}

impl VehicleSelector {
    /// The threshold in the canonical unit of the dimension (metres, tonnes, or a count).
    ///
    /// A missing unit on a length or weight is read as metres or tonnes. A unit that
    /// measures the wrong quantity (feet on a weight) is an error, with the reason.
    pub fn canonical_value(&self) -> Result<f64, String> {
        let quantity = self.dimension.quantity();
        match self.unit {
            None => Ok(self.value),
            Some(u) if u.quantity() == quantity => Ok(u.to_canonical(self.value)),
            Some(u) => Err(format!("unit {u} cannot measure {}", self.dimension)),
        }
    }
}

impl fmt::Display for VehicleSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {}",
            self.dimension,
            self.comparison.symbol(),
            self.value
        )?;
        if let Some(u) = self.unit {
            write!(f, " {u}")?;
        }
        Ok(())
    }
}

/// The `when` block of a rule: conditions on the traveller, all of which must hold.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct When {
    /// Direction of travel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<Heading>,
    /// Time of travel, in OpenStreetMap `opening_hours` syntax.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub during: Option<String>,
    /// Purposes of use, any of which qualifies (`as_customer`, `to_deliver`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub using: Option<Vec<String>>,
    /// Statuses, any of which qualifies (`as_private`, `as_permitted`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recognized: Option<Vec<String>>,
    /// Travel modes, any of which qualifies (including their descendants).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<Vec<TravelMode>>,
    /// Vehicle conditions, all of which must hold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vehicle: Option<Vec<VehicleSelector>>,
}

impl When {
    /// Whether the block sets no condition at all.
    pub fn is_empty(&self) -> bool {
        self.heading.is_none()
            && self.during.is_none()
            && self.using.is_none()
            && self.recognized.is_none()
            && self.mode.is_none()
            && self.vehicle.is_none()
    }
}

/// Renders as `{heading: backward, mode: [motor_vehicle]}`.
impl fmt::Display for When {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn list<T: fmt::Display>(items: &[T]) -> String {
            items
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        }
        let mut parts = Vec::new();
        if let Some(h) = self.heading {
            parts.push(format!("heading: {h}"));
        }
        if let Some(d) = &self.during {
            parts.push(format!("during: \"{d}\""));
        }
        if let Some(m) = &self.mode {
            parts.push(format!("mode: [{}]", list(m)));
        }
        if let Some(u) = &self.using {
            parts.push(format!("using: [{}]", list(u)));
        }
        if let Some(r) = &self.recognized {
            parts.push(format!("recognized: [{}]", list(r)));
        }
        if let Some(v) = &self.vehicle {
            parts.push(format!("vehicle: [{}]", list(v)));
        }
        write!(f, "{{{}}}", parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taxonomy() {
        assert!(TravelMode::Car.is_within(&TravelMode::Vehicle));
        assert!(TravelMode::Car.is_within(&TravelMode::MotorVehicle));
        assert!(TravelMode::Bus.is_within(&TravelMode::MotorVehicle));
        assert!(TravelMode::Bicycle.is_within(&TravelMode::Vehicle));
        assert!(!TravelMode::Bicycle.is_within(&TravelMode::MotorVehicle));
        assert!(!TravelMode::Foot.is_within(&TravelMode::Vehicle));
        assert!(!TravelMode::MotorVehicle.is_within(&TravelMode::Car));
    }

    #[test]
    fn unknown_modes_survive_deserialisation_but_not_parsing() {
        let m: TravelMode = serde_json::from_str("\"tractor\"").unwrap();
        assert_eq!(m, TravelMode::Other("tractor".into()));
        assert_eq!(m.lineage(), std::slice::from_ref(&m));
        assert_eq!(serde_json::to_string(&m).unwrap(), "\"tractor\"");
        assert!("tractor".parse::<TravelMode>().is_err());
        assert_eq!("hgv".parse::<TravelMode>().unwrap(), TravelMode::Hgv);
    }

    #[test]
    fn units_convert_to_metres_and_tonnes() {
        assert!((Unit::Ft.to_canonical(10.0) - 3.048).abs() < 1e-12);
        assert!((Unit::Lb.to_canonical(2_000.0) - Unit::St.to_canonical(1.0)).abs() < 1e-12);
        let sel = VehicleSelector {
            dimension: Dimension::Weight,
            comparison: Comparison::GreaterThan,
            value: 7_500.0,
            unit: Some(Unit::Kg),
        };
        assert!((sel.canonical_value().unwrap() - 7.5).abs() < 1e-12);
        let wrong = VehicleSelector {
            unit: Some(Unit::Ft),
            ..sel
        };
        assert!(wrong.canonical_value().is_err());
    }

    #[test]
    fn comparisons_are_not_flipped_by_unit_rounding() {
        let twelve_ft = Unit::Ft.to_canonical(12.0);
        assert_ne!(twelve_ft, 3.6576, "the rounding this test guards against");
        for op in [Comparison::LessThan, Comparison::GreaterThan] {
            assert!(!op.holds(3.6576, twelve_ft), "{op:?}");
            assert!(!op.holds(twelve_ft, 3.6576), "{op:?}");
        }
        for op in [
            Comparison::Equal,
            Comparison::LessThanEqual,
            Comparison::GreaterThanEqual,
        ] {
            assert!(op.holds(3.6576, twelve_ft), "{op:?}");
        }
        assert!(Comparison::GreaterThan.holds(3.66, twelve_ft));
        assert!(Comparison::LessThan.holds(3.65, twelve_ft));
    }

    #[test]
    fn when_renders_compactly() {
        let w = When {
            heading: Some(Heading::Backward),
            mode: Some(vec![TravelMode::MotorVehicle]),
            ..When::default()
        };
        assert_eq!(w.to_string(), "{heading: backward, mode: [motor_vehicle]}");
    }
}
