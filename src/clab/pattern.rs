use std::collections::HashSet;

/// A single segment in an interface pattern, e.g. "ethernet-1/{n:1-24}"
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternSet {
    pub prefix: String,
    pub suffix: String,
    pub start_index: u32,
    pub end_index: Option<u32>,
}

impl PatternSet {
    pub fn new(
        prefix: impl Into<String>,
        suffix: impl Into<String>,
        start_index: u32,
        end_index: Option<u32>,
    ) -> Self {
        Self {
            prefix: prefix.into(),
            suffix: suffix.into(),
            start_index,
            end_index,
        }
    }

    /// Format interface name for a given 0-based offset or absolute index
    pub fn format_index(&self, index: u32) -> String {
        format!("{}{}{}", self.prefix, index, self.suffix)
    }

    /// Check if a port matches this pattern set and extract the index number
    pub fn extract_index(&self, port: &str) -> Option<u32> {
        if !port.starts_with(&self.prefix) || !port.ends_with(&self.suffix) {
            return None;
        }
        let start_len = self.prefix.len();
        let end_len = port.len().checked_sub(self.suffix.len())?;
        if start_len > end_len {
            return None;
        }
        let num_str = &port[start_len..end_len];
        num_str.parse::<u32>().ok()
    }
}

/// Interface pattern parser and auto-allocator matching vscode-containerlab syntax:
/// e.g. "ethernet-1/{n:1-24},ethernet-2/{n:1-24}" or "e1-{n}" or "1/1/c{n}/1"
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfacePattern {
    pub raw: String,
    pub sets: Vec<PatternSet>,
}

impl InterfacePattern {
    pub const DEFAULT_PATTERN: &'static str = "eth{n:1}";

    /// Parse pattern string with optional {n}, {n:start}, and {n:start-end} syntax,
    /// supporting comma-separated spillover sets.
    pub fn parse(raw_pattern: &str) -> Self {
        let raw = raw_pattern.trim();
        if raw.is_empty() {
            return Self::parse(Self::DEFAULT_PATTERN);
        }

        let parts = Self::split_pattern_sets(raw);
        let mut sets = Vec::new();

        for part in parts {
            sets.push(Self::parse_single_set(&part));
        }

        if sets.is_empty() {
            sets.push(Self::parse_single_set(Self::DEFAULT_PATTERN));
        }

        Self {
            raw: raw.to_string(),
            sets,
        }
    }

    /// Validate interface pattern syntax. Returns Ok(()) if valid, or an error message.
    pub fn validate(pattern_str: &str) -> Result<(), String> {
        let trimmed = pattern_str.trim();
        if trimmed.is_empty() {
            return Err("Interface pattern cannot be empty".to_string());
        }

        let sets = Self::split_pattern_sets(trimmed);
        if sets.is_empty() {
            return Err("Interface pattern must contain at least one definition".to_string());
        }

        for part in sets {
            let part_trimmed = part.trim();
            if part_trimmed.is_empty() {
                return Err("Pattern segment cannot be empty".to_string());
            }

            let mut in_brace = false;
            let mut brace_content = String::new();
            let mut has_brace = false;

            for ch in part_trimmed.chars() {
                if ch == '{' {
                    if in_brace || has_brace {
                        return Err("Multiple or nested variable placeholders '{...}' are not allowed in a segment".to_string());
                    }
                    in_brace = true;
                    has_brace = true;
                } else if ch == '}' {
                    if !in_brace {
                        return Err("Unmatched closing brace '}'".to_string());
                    }
                    in_brace = false;
                } else if in_brace {
                    brace_content.push(ch);
                } else if !ch.is_ascii_alphanumeric()
                    && ch != '/'
                    && ch != '-'
                    && ch != '_'
                    && ch != '.'
                    && ch != ':'
                {
                    return Err(format!(
                        "Invalid character '{}' in pattern prefix/suffix",
                        ch
                    ));
                }
            }

            if in_brace {
                return Err("Unclosed opening brace '{'".to_string());
            }

            if has_brace {
                if !brace_content.starts_with('n') {
                    return Err(
                        "Variable placeholder must start with 'n', e.g. {n}, {n:1}, or {n:1-24}"
                            .to_string(),
                    );
                }
                let rest = &brace_content[1..];
                if !rest.is_empty() {
                    if !rest.starts_with(':') {
                        return Err(
                            "Variable placeholder format is {n}, {n:start}, or {n:start-end}"
                                .to_string(),
                        );
                    }
                    let range = &rest[1..];
                    if let Some(dash_idx) = range.find('-') {
                        let start_str = &range[..dash_idx];
                        let end_str = &range[dash_idx + 1..];
                        let start = start_str
                            .parse::<u32>()
                            .map_err(|_| format!("Invalid start index '{}'", start_str))?;
                        let end = end_str
                            .parse::<u32>()
                            .map_err(|_| format!("Invalid end index '{}'", end_str))?;
                        if start > end {
                            return Err(format!(
                                "Start index {} cannot be greater than end index {}",
                                start, end
                            ));
                        }
                    } else {
                        range
                            .parse::<u32>()
                            .map_err(|_| format!("Invalid start index '{}'", range))?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Split comma-separated sets respecting brace nesting
    fn split_pattern_sets(pattern_list: &str) -> Vec<String> {
        let mut patterns = Vec::new();
        let mut current = String::new();
        let mut brace_depth: usize = 0;

        for ch in pattern_list.chars() {
            if ch == '{' {
                brace_depth += 1;
            } else if ch == '}' {
                brace_depth = brace_depth.saturating_sub(1);
            }

            if ch == ',' && brace_depth == 0 {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    patterns.push(trimmed.to_string());
                }
                current.clear();
                continue;
            }

            current.push(ch);
        }

        let trimmed = current.trim();
        if !trimmed.is_empty() {
            patterns.push(trimmed.to_string());
        }

        patterns
    }

    /// Parse a single set pattern like "ethernet-1/{n:1-24}" or "e1-{n}" or "eth"
    fn parse_single_set(part: &str) -> PatternSet {
        let trimmed = part.trim();
        // Look for "{n" and closing "}"
        if let Some(open_idx) = trimmed.find("{n") {
            if let Some(close_idx) = trimmed[open_idx..].find('}') {
                let close_idx = open_idx + close_idx;
                let prefix = &trimmed[..open_idx];
                let suffix = &trimmed[close_idx + 1..];
                let spec = &trimmed[open_idx + 2..close_idx]; // after "{n" up to "}"

                let (start_idx, end_idx) = if let Some(range_spec) = spec.strip_prefix(':') {
                    if let Some(dash_idx) = range_spec.find('-') {
                        let start = range_spec[..dash_idx].parse::<u32>().unwrap_or(1);
                        let end = range_spec[dash_idx + 1..].parse::<u32>().ok();
                        (start, end)
                    } else {
                        let start = range_spec.parse::<u32>().unwrap_or(1);
                        (start, None)
                    }
                } else {
                    (1, None)
                };

                let end_idx = match end_idx {
                    Some(e) if e >= start_idx => Some(e),
                    _ => None,
                };

                return PatternSet::new(prefix, suffix, start_idx, end_idx);
            }
        }

        // Fallback: entire string as prefix
        PatternSet::new(
            if trimmed.is_empty() { "eth" } else { trimmed },
            "",
            1,
            None,
        )
    }

    /// Allocate next available interface name that is not present in `used_interfaces`
    pub fn allocate_next(&self, used_interfaces: &[&str]) -> String {
        let used_set: HashSet<&str> = used_interfaces.iter().copied().collect();

        // 1. Try each pattern set in order within its bounds
        for set in &self.sets {
            let max = set.end_index.unwrap_or(u32::MAX);
            let mut idx = set.start_index;
            while idx <= max {
                let candidate = set.format_index(idx);
                if !used_set.contains(candidate.as_str()) {
                    return candidate;
                }
                if idx == u32::MAX {
                    break;
                }
                idx += 1;
            }
        }

        // 2. Spillover: if all finite sets are exhausted, spill into the last set unboundedly
        if let Some(last_set) = self.sets.last() {
            let mut idx = last_set
                .end_index
                .map_or(last_set.start_index, |e| e.saturating_add(1));
            loop {
                let candidate = last_set.format_index(idx);
                if !used_set.contains(candidate.as_str()) {
                    return candidate;
                }
                if idx == u32::MAX {
                    break;
                }
                idx += 1;
            }
        }

        // Ultimate fallback
        format!("eth{}", used_interfaces.len() + 1)
    }
}
