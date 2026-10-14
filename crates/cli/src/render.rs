//! Small text-formatting helpers shared by the reports.

use overrules::Segment;

/// The first 13 characters of a GERS id: enough to be unique in a city.
pub fn short(id: &str) -> &str {
    id.get(..13).unwrap_or(id)
}

/// Truncates to `n` characters, marking the cut with an ellipsis.
pub fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        let cut: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

/// `""` for one, `"s"` otherwise.
pub fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// `access×2 speed×1 no-turn×3 LR`: which rule kinds a segment carries.
pub fn rule_flags(s: &Segment) -> String {
    let mut f = Vec::new();
    if !s.access_restrictions.is_empty() {
        f.push(format!("access×{}", s.access_restrictions.len()));
    }
    if !s.speed_limits.is_empty() {
        f.push(format!("speed×{}", s.speed_limits.len()));
    }
    if !s.prohibited_transitions.is_empty() {
        f.push(format!("no-turn×{}", s.prohibited_transitions.len()));
    }
    if s.has_between() {
        f.push("LR".to_owned());
    }
    f.join(" ")
}
