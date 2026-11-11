//! Glob patterns over `section.metric` names.

/// Returns true when `name` matches `pattern`, where `*` matches any run of characters.
pub fn matches(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == name;
    }
    let mut rest = name;
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            let Some(r) = rest.strip_prefix(part) else {
                return false;
            };
            rest = r;
        } else if i == parts.len() - 1 {
            return rest.ends_with(part);
        } else {
            let Some(at) = rest.find(part) else {
                return false;
            };
            rest = &rest[at + part.len()..];
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs() {
        assert!(matches("latency.total_p50", "latency.total_p50"));
        assert!(!matches("latency.total_p50", "latency.total_p95"));
        assert!(matches("latency.*", "latency.total_p95"));
        assert!(matches("*.jank_count", "frame_pacing.jank_count"));
        assert!(matches("*pacing*count", "frame_pacing.jank_count"));
        assert!(!matches("*pacing*fps", "frame_pacing.jank_count"));
        assert!(matches("*", "anything"));
    }
}
