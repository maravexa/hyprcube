use std::fmt;
use std::path::Path;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// A parsed hyprland.conf file that preserves comments, blank lines, and
/// ordering for lossless roundtrip editing.
#[derive(Debug, Clone, PartialEq)]
pub struct HyprlandConfig {
    pub items: Vec<ConfigItem>,
}

/// A single entry inside a hyprland.conf file.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigItem {
    /// An empty / whitespace-only line.
    Blank,
    /// A comment line (stored including the leading `#`).
    Comment(String),
    /// A `key = value` assignment.
    KeyValue { key: String, value: String },
    /// A `name { ... }` section block (may be nested).
    Section {
        name: String,
        items: Vec<ConfigItem>,
    },
    /// A `source = path` include directive.
    Source(String),
}

/// Errors that can occur while parsing a hyprland.conf file.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unexpected closing brace at line {0}")]
    UnexpectedCloseBrace(usize),
    #[error("unclosed section starting at line {0}")]
    UnclosedSection(usize),
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

impl HyprlandConfig {
    /// Parse a hyprland.conf from a string.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        // Stack entries: (parent items, section name, opening line number)
        let mut stack: Vec<(Vec<ConfigItem>, String, usize)> = Vec::new();
        let mut items: Vec<ConfigItem> = Vec::new();

        for (idx, line) in input.lines().enumerate() {
            let line_num = idx + 1;
            let trimmed = line.trim();

            if trimmed.is_empty() {
                items.push(ConfigItem::Blank);
            } else if trimmed.starts_with('#') {
                items.push(ConfigItem::Comment(trimmed.to_string()));
            } else if trimmed == "}" {
                let (mut parent_items, name, _open_line) = stack
                    .pop()
                    .ok_or(ParseError::UnexpectedCloseBrace(line_num))?;
                parent_items.push(ConfigItem::Section { name, items });
                items = parent_items;
            } else if let Some(name) = trimmed.strip_suffix('{') {
                let name = name.trim().to_string();
                stack.push((items, name, line_num));
                items = Vec::new();
            } else if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim().to_string();
                let value = trimmed[eq_pos + 1..].trim().to_string();

                if key == "source" {
                    items.push(ConfigItem::Source(value));
                } else {
                    items.push(ConfigItem::KeyValue { key, value });
                }
            } else {
                // Bare keyword (no `=`), store as key with empty value.
                items.push(ConfigItem::KeyValue {
                    key: trimmed.to_string(),
                    value: String::new(),
                });
            }
        }

        if let Some((_, _, open_line)) = stack.last() {
            return Err(ParseError::UnclosedSection(*open_line));
        }

        Ok(HyprlandConfig { items })
    }

    /// Load and parse a hyprland.conf from a file path.
    pub fn load(path: &Path) -> Result<Self, ParseError> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Load from the default location (`$XDG_CONFIG_HOME/hypr/hyprland.conf`,
    /// falling back to `~/.config/hypr/hyprland.conf`).
    pub fn load_default() -> Result<Self, ParseError> {
        let config_dir = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME environment variable not set");
            format!("{home}/.config")
        });
        let path = Path::new(&config_dir).join("hypr/hyprland.conf");
        Self::load(&path)
    }
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

impl HyprlandConfig {
    /// Serialize back to hyprland.conf format.
    ///
    /// Indentation is normalized to 4 spaces per nesting level.  Comments and
    /// ordering are preserved.
    pub fn to_string_lossy(&self) -> String {
        let mut buf = String::new();
        write_items(&mut buf, &self.items, 0);
        buf
    }
}

fn write_items(buf: &mut String, items: &[ConfigItem], indent: usize) {
    let prefix = " ".repeat(indent);
    for item in items {
        match item {
            ConfigItem::Blank => buf.push('\n'),
            ConfigItem::Comment(c) => {
                buf.push_str(&prefix);
                buf.push_str(c);
                buf.push('\n');
            }
            ConfigItem::KeyValue { key, value } => {
                buf.push_str(&prefix);
                if value.is_empty() {
                    buf.push_str(key);
                } else {
                    buf.push_str(key);
                    buf.push_str(" = ");
                    buf.push_str(value);
                }
                buf.push('\n');
            }
            ConfigItem::Section { name, items: sub } => {
                buf.push_str(&prefix);
                buf.push_str(name);
                buf.push_str(" {\n");
                write_items(buf, sub, indent + 4);
                buf.push_str(&prefix);
                buf.push_str("}\n");
            }
            ConfigItem::Source(path) => {
                buf.push_str(&prefix);
                buf.push_str("source = ");
                buf.push_str(path);
                buf.push('\n');
            }
        }
    }
}

impl fmt::Display for HyprlandConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string_lossy())
    }
}

// ---------------------------------------------------------------------------
// Query API
// ---------------------------------------------------------------------------

impl HyprlandConfig {
    /// Get the value of the first top-level `key = value` matching `key`.
    pub fn get(&self, key: &str) -> Option<&str> {
        get_in_items(&self.items, key)
    }

    /// Get all values for a given top-level key (e.g. `bind`, `monitor`).
    pub fn get_all(&self, key: &str) -> Vec<&str> {
        get_all_in_items(&self.items, key)
    }

    /// Get a value inside a named top-level section.
    pub fn get_in_section(&self, section: &str, key: &str) -> Option<&str> {
        for item in &self.items {
            if let ConfigItem::Section { name, items } = item {
                if name == section {
                    if let Some(v) = get_in_items(items, key) {
                        return Some(v);
                    }
                }
            }
        }
        None
    }

    /// Get a value inside arbitrarily nested sections.
    ///
    /// ```ignore
    /// config.get_in_nested(&["decoration", "blur"], "enabled")
    /// ```
    pub fn get_in_nested(&self, path: &[&str], key: &str) -> Option<&str> {
        let mut current = &self.items;
        for section_name in path {
            let mut found = false;
            for item in current {
                if let ConfigItem::Section { name, items } = item {
                    if name == *section_name {
                        current = items;
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                return None;
            }
        }
        get_in_items(current, key)
    }
}

fn get_in_items<'a>(items: &'a [ConfigItem], key: &str) -> Option<&'a str> {
    for item in items {
        if let ConfigItem::KeyValue { key: k, value } = item {
            if k == key {
                return Some(value.as_str());
            }
        }
    }
    None
}

fn get_all_in_items<'a>(items: &'a [ConfigItem], key: &str) -> Vec<&'a str> {
    items
        .iter()
        .filter_map(|item| match item {
            ConfigItem::KeyValue { key: k, value } if k == key => Some(value.as_str()),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Mutation API
// ---------------------------------------------------------------------------

impl HyprlandConfig {
    /// Update the first top-level entry matching `key`, or append a new one.
    pub fn set(&mut self, key: &str, value: &str) {
        for item in self.items.iter_mut() {
            if let ConfigItem::KeyValue { key: k, value: v } = item {
                if k == key {
                    *v = value.to_string();
                    return;
                }
            }
        }
        self.items.push(ConfigItem::KeyValue {
            key: key.to_string(),
            value: value.to_string(),
        });
    }

    /// Update a key inside a top-level section, creating the section if needed.
    pub fn set_in_section(&mut self, section: &str, key: &str, value: &str) {
        for item in self.items.iter_mut() {
            if let ConfigItem::Section { name, items } = item {
                if name == section {
                    for sub in items.iter_mut() {
                        if let ConfigItem::KeyValue { key: k, value: v } = sub {
                            if k == key {
                                *v = value.to_string();
                                return;
                            }
                        }
                    }
                    items.push(ConfigItem::KeyValue {
                        key: key.to_string(),
                        value: value.to_string(),
                    });
                    return;
                }
            }
        }
        // Section does not exist — create it.
        self.items.push(ConfigItem::Section {
            name: section.to_string(),
            items: vec![ConfigItem::KeyValue {
                key: key.to_string(),
                value: value.to_string(),
            }],
        });
    }

    /// Update a key inside a nested section path, creating sections as needed.
    pub fn set_in_nested(&mut self, path: &[&str], key: &str, value: &str) {
        set_in_nested_items(&mut self.items, path, key, value);
    }

    /// Always append a new `key = value` entry (for multi-value keys like
    /// `bind`).
    pub fn add(&mut self, key: &str, value: &str) {
        self.items.push(ConfigItem::KeyValue {
            key: key.to_string(),
            value: value.to_string(),
        });
    }

    /// Remove all top-level entries matching `key`.  If `value_prefix` is
    /// provided, only remove entries whose value starts with that prefix.
    pub fn remove_all(&mut self, key: &str, value_prefix: Option<&str>) {
        self.items.retain(|item| {
            if let ConfigItem::KeyValue { key: k, value: v } = item {
                if k == key {
                    return match value_prefix {
                        Some(prefix) => !v.starts_with(prefix),
                        None => false,
                    };
                }
            }
            true
        });
    }

    /// Remove the first top-level `key = value` matching `key`.
    /// Returns `true` if an entry was removed.
    pub fn remove(&mut self, key: &str) -> bool {
        let mut removed = false;
        self.items.retain(|item| {
            if !removed {
                if let ConfigItem::KeyValue { key: k, .. } = item {
                    if k == key {
                        removed = true;
                        return false;
                    }
                }
            }
            true
        });
        removed
    }

    /// Remove the first `key = value` inside the named top-level section.
    /// Returns `true` if an entry was removed.
    pub fn remove_in_section(&mut self, section: &str, key: &str) -> bool {
        for item in self.items.iter_mut() {
            if let ConfigItem::Section { name, items } = item {
                if name == section {
                    let mut removed = false;
                    items.retain(|sub| {
                        if !removed {
                            if let ConfigItem::KeyValue { key: k, .. } = sub {
                                if k == key {
                                    removed = true;
                                    return false;
                                }
                            }
                        }
                        true
                    });
                    return removed;
                }
            }
        }
        false
    }
}

fn set_in_nested_items(items: &mut Vec<ConfigItem>, path: &[&str], key: &str, value: &str) {
    if path.is_empty() {
        // Update existing or append.
        for item in items.iter_mut() {
            if let ConfigItem::KeyValue { key: k, value: v } = item {
                if k == key {
                    *v = value.to_string();
                    return;
                }
            }
        }
        items.push(ConfigItem::KeyValue {
            key: key.to_string(),
            value: value.to_string(),
        });
        return;
    }

    let section_name = path[0];
    let rest = &path[1..];

    // Try to find the section and recurse.
    for item in items.iter_mut() {
        if let ConfigItem::Section { name, items: sub } = item {
            if name == section_name {
                set_in_nested_items(sub, rest, key, value);
                return;
            }
        }
    }

    // Section does not exist — build it recursively.
    let mut new_items = Vec::new();
    set_in_nested_items(&mut new_items, rest, key, value);
    items.push(ConfigItem::Section {
        name: section_name.to_string(),
        items: new_items,
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A realistic config snippet used across several tests.
    fn sample_config() -> &'static str {
        "\
# HyprCube test config
monitor = DP-1, 1920x1080@144, 0x0, 1
monitor = HDMI-A-1, 2560x1440@60, 1920x0, 1

general {
    gaps_in = 5
    gaps_out = 10
    border_size = 2
    col.active_border = rgba(89b4faee)
}

decoration {
    rounding = 10
    blur {
        enabled = true
        size = 3
    }
}

# Keybindings
bind = SUPER, Return, exec, kitty
bind = SUPER, Q, killactive,
bind = SUPER, M, exit,
bind = SUPER, V, togglefloating,
bind = SUPER, P, pseudo,

source = ~/.config/hypr/keybinds.conf"
    }

    // -----------------------------------------------------------------------
    // 1. Roundtrip test
    // -----------------------------------------------------------------------

    #[test]
    fn roundtrip_preserves_structure() {
        let input = sample_config();
        let config = HyprlandConfig::parse(input).unwrap();
        let output = config.to_string_lossy();

        // Re-parse the output and compare ASTs.
        let reparsed = HyprlandConfig::parse(&output).unwrap();
        assert_eq!(config, reparsed, "AST must survive a roundtrip");

        // Also verify line-by-line that comments and blanks are in order.
        let in_lines: Vec<&str> = input.lines().collect();
        let out_lines: Vec<&str> = output.lines().collect();
        assert_eq!(in_lines.len(), out_lines.len(), "line count must match");

        for (i, (a, b)) in in_lines.iter().zip(out_lines.iter()).enumerate() {
            assert_eq!(
                a.trim(),
                b.trim(),
                "content mismatch at line {} (whitespace normalization is ok)",
                i + 1
            );
        }
    }

    // -----------------------------------------------------------------------
    // 2. Nested section test
    // -----------------------------------------------------------------------

    #[test]
    fn nested_section_query() {
        let input = "\
decoration {
    rounding = 10
    blur {
        enabled = true
        size = 3
    }
}";
        let config = HyprlandConfig::parse(input).unwrap();

        assert_eq!(
            config.get_in_nested(&["decoration"], "rounding"),
            Some("10")
        );
        assert_eq!(
            config.get_in_nested(&["decoration", "blur"], "enabled"),
            Some("true")
        );
        assert_eq!(
            config.get_in_nested(&["decoration", "blur"], "size"),
            Some("3")
        );
        assert_eq!(
            config.get_in_nested(&["decoration", "blur"], "missing"),
            None
        );
        assert_eq!(
            config.get_in_nested(&["nonexistent", "blur"], "enabled"),
            None
        );
    }

    // -----------------------------------------------------------------------
    // 3. Multi-value test
    // -----------------------------------------------------------------------

    #[test]
    fn get_all_returns_all_values() {
        let input = "\
bind = SUPER, Return, exec, kitty
bind = SUPER, Q, killactive,
bind = SUPER, M, exit,
bind = SUPER, V, togglefloating,
bind = SUPER, P, pseudo,";
        let config = HyprlandConfig::parse(input).unwrap();

        let binds = config.get_all("bind");
        assert_eq!(binds.len(), 5);
        assert_eq!(binds[0], "SUPER, Return, exec, kitty");
        assert_eq!(binds[4], "SUPER, P, pseudo,");
    }

    // -----------------------------------------------------------------------
    // 4. Mutation tests
    // -----------------------------------------------------------------------

    #[test]
    fn set_in_section_updates_existing_key() {
        let input = "\
general {
    gaps_in = 5
    gaps_out = 10
}";
        let mut config = HyprlandConfig::parse(input).unwrap();
        config.set_in_section("general", "gaps_in", "8");

        assert_eq!(config.get_in_section("general", "gaps_in"), Some("8"));
        // Other key untouched.
        assert_eq!(config.get_in_section("general", "gaps_out"), Some("10"));
    }

    #[test]
    fn set_in_section_creates_section_if_missing() {
        let mut config = HyprlandConfig::parse("").unwrap();
        config.set_in_section("input", "kb_layout", "us");

        assert_eq!(config.get_in_section("input", "kb_layout"), Some("us"));
        // Verify it appears in the serialized output.
        let output = config.to_string_lossy();
        assert!(output.contains("input {"));
        assert!(output.contains("kb_layout = us"));
    }

    #[test]
    fn set_in_nested_creates_path() {
        let mut config = HyprlandConfig::parse("").unwrap();
        config.set_in_nested(&["decoration", "blur"], "enabled", "true");

        assert_eq!(
            config.get_in_nested(&["decoration", "blur"], "enabled"),
            Some("true")
        );
    }

    #[test]
    fn set_updates_first_match() {
        let input = "\
monitor = DP-1, 1920x1080@144, 0x0, 1
exec-once = waybar";
        let mut config = HyprlandConfig::parse(input).unwrap();
        config.set("exec-once", "eww open bar");
        assert_eq!(config.get("exec-once"), Some("eww open bar"));
    }

    #[test]
    fn set_appends_when_missing() {
        let mut config = HyprlandConfig::parse("").unwrap();
        config.set("exec-once", "waybar");
        assert_eq!(config.get("exec-once"), Some("waybar"));
    }

    // -----------------------------------------------------------------------
    // 5. Remove test
    // -----------------------------------------------------------------------

    #[test]
    fn remove_all_by_prefix() {
        let mut config = HyprlandConfig::parse("").unwrap();
        config.add("bind", "SUPER, Return, exec, kitty");
        config.add("bind", "SUPER, Q, killactive,");
        config.add("bind", "SUPER, M, exit,");

        assert_eq!(config.get_all("bind").len(), 3);

        config.remove_all("bind", Some("SUPER, Q"));
        let binds = config.get_all("bind");
        assert_eq!(binds.len(), 2);
        assert_eq!(binds[0], "SUPER, Return, exec, kitty");
        assert_eq!(binds[1], "SUPER, M, exit,");
    }

    #[test]
    fn remove_all_without_prefix() {
        let mut config = HyprlandConfig::parse("").unwrap();
        config.add("bind", "SUPER, Return, exec, kitty");
        config.add("bind", "SUPER, Q, killactive,");

        config.remove_all("bind", None);
        assert!(config.get_all("bind").is_empty());
    }

    // -----------------------------------------------------------------------
    // 6. Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn empty_input() {
        let config = HyprlandConfig::parse("").unwrap();
        assert!(config.items.is_empty());
        assert_eq!(config.to_string_lossy(), "");
    }

    #[test]
    fn section_with_no_contents() {
        let input = "empty_section {\n}";
        let config = HyprlandConfig::parse(input).unwrap();
        assert_eq!(config.items.len(), 1);
        if let ConfigItem::Section { name, items } = &config.items[0] {
            assert_eq!(name, "empty_section");
            assert!(items.is_empty());
        } else {
            panic!("expected Section");
        }
    }

    #[test]
    fn value_containing_equals_sign() {
        let input = "env = GDK_BACKEND=wayland,x11";
        let config = HyprlandConfig::parse(input).unwrap();
        assert_eq!(config.get("env"), Some("GDK_BACKEND=wayland,x11"));
    }

    #[test]
    fn comment_after_value_preserved() {
        // Hyprland treats `#` after a value as an inline comment, but our
        // structure-preserving parser keeps the full text as the value.
        let input = "gaps_in = 5 # default gap";
        let config = HyprlandConfig::parse(input).unwrap();
        assert_eq!(config.get("gaps_in"), Some("5 # default gap"));

        // Roundtrip preserves it.
        let output = config.to_string_lossy();
        assert_eq!(output.trim(), input.trim());
    }

    #[test]
    fn source_directive_parsed() {
        let input = "source = ~/.config/hypr/keybinds.conf";
        let config = HyprlandConfig::parse(input).unwrap();
        assert_eq!(config.items.len(), 1);
        assert!(
            matches!(&config.items[0], ConfigItem::Source(p) if p == "~/.config/hypr/keybinds.conf")
        );
        // Not returned by `get("source")`.
        assert_eq!(config.get("source"), None);
    }

    #[test]
    fn unexpected_close_brace() {
        let err = HyprlandConfig::parse("}").unwrap_err();
        assert!(
            matches!(err, ParseError::UnexpectedCloseBrace(1)),
            "expected UnexpectedCloseBrace(1), got {err:?}"
        );
    }

    #[test]
    fn unclosed_section() {
        let err = HyprlandConfig::parse("general {\n  gaps_in = 5").unwrap_err();
        assert!(
            matches!(err, ParseError::UnclosedSection(1)),
            "expected UnclosedSection(1), got {err:?}"
        );
    }

    #[test]
    fn deeply_nested_sections() {
        let input = "\
a {
    b {
        c {
            key = deep
        }
    }
}";
        let config = HyprlandConfig::parse(input).unwrap();
        assert_eq!(config.get_in_nested(&["a", "b", "c"], "key"), Some("deep"));

        let output = config.to_string_lossy();
        let reparsed = HyprlandConfig::parse(&output).unwrap();
        assert_eq!(config, reparsed);
    }

    #[test]
    fn get_in_section_convenience() {
        let config = HyprlandConfig::parse(sample_config()).unwrap();
        assert_eq!(config.get_in_section("general", "border_size"), Some("2"));
        assert_eq!(
            config.get_in_section("general", "col.active_border"),
            Some("rgba(89b4faee)")
        );
        assert_eq!(config.get_in_section("general", "missing"), None);
        assert_eq!(config.get_in_section("missing", "gaps_in"), None);
    }

    #[test]
    fn add_always_appends() {
        let mut config = HyprlandConfig::parse("bind = SUPER, A, exec, a").unwrap();
        config.add("bind", "SUPER, B, exec, b");
        let binds = config.get_all("bind");
        assert_eq!(binds.len(), 2);
        assert_eq!(binds[1], "SUPER, B, exec, b");
    }

    #[test]
    fn set_in_nested_updates_existing() {
        let input = "\
decoration {
    blur {
        enabled = true
    }
}";
        let mut config = HyprlandConfig::parse(input).unwrap();
        config.set_in_nested(&["decoration", "blur"], "enabled", "false");
        assert_eq!(
            config.get_in_nested(&["decoration", "blur"], "enabled"),
            Some("false")
        );
    }

    #[test]
    fn multiple_source_directives() {
        let input = "\
source = ~/.config/hypr/a.conf
source = ~/.config/hypr/b.conf";
        let config = HyprlandConfig::parse(input).unwrap();
        let sources: Vec<&str> = config
            .items
            .iter()
            .filter_map(|i| match i {
                ConfigItem::Source(p) => Some(p.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            sources,
            vec!["~/.config/hypr/a.conf", "~/.config/hypr/b.conf",]
        );
    }

    #[test]
    fn blank_lines_preserved() {
        let input = "a = 1\n\n\nb = 2";
        let config = HyprlandConfig::parse(input).unwrap();
        // a=1, blank, blank, b=2
        assert_eq!(config.items.len(), 4);
        assert_eq!(config.items[1], ConfigItem::Blank);
        assert_eq!(config.items[2], ConfigItem::Blank);
    }

    // -----------------------------------------------------------------------
    // 7. Remove API tests
    // -----------------------------------------------------------------------

    #[test]
    fn remove_first_top_level_key() {
        let mut config = HyprlandConfig::parse("a = 1\nb = 2\na = 3").unwrap();
        let removed = config.remove("a");
        assert!(removed);
        // Only the first "a" is gone; second "a = 3" remains.
        assert_eq!(config.get("a"), Some("3"));
        assert_eq!(config.get("b"), Some("2"));
    }

    #[test]
    fn remove_nonexistent_key_returns_false() {
        let mut config = HyprlandConfig::parse("a = 1").unwrap();
        assert!(!config.remove("missing"));
    }

    #[test]
    fn remove_in_section_removes_matching_key() {
        let input = "general {\n    gaps_in = 5\n    gaps_out = 10\n}";
        let mut config = HyprlandConfig::parse(input).unwrap();
        let removed = config.remove_in_section("general", "gaps_in");
        assert!(removed);
        assert_eq!(config.get_in_section("general", "gaps_in"), None);
        assert_eq!(config.get_in_section("general", "gaps_out"), Some("10"));
    }

    #[test]
    fn remove_in_section_missing_section_returns_false() {
        let mut config = HyprlandConfig::parse("general {\n    gaps_in = 5\n}").unwrap();
        assert!(!config.remove_in_section("input", "kb_layout"));
    }

    #[test]
    fn remove_in_section_missing_key_returns_false() {
        let mut config = HyprlandConfig::parse("general {\n    gaps_in = 5\n}").unwrap();
        assert!(!config.remove_in_section("general", "nonexistent"));
    }

    // -----------------------------------------------------------------------
    // 8. Roundtrip safety: panel save must not introduce empty-value keys
    // -----------------------------------------------------------------------

    #[test]
    fn save_does_not_add_empty_keys() {
        let input = "\
general {
    gaps_in = 5
    border_size = 2
}

input {
    kb_layout = us
    sensitivity = 0.0
}
";
        let mut config = HyprlandConfig::parse(input).unwrap();

        // Simulate what a panel save does — set only dirty fields
        config.set_in_section("general", "gaps_in", "10");
        // Do NOT set kb_variant, kb_model, etc. — they were never in the config.

        let output = config.to_string_lossy();

        // Verify no empty-value keys were introduced.
        for line in output.lines() {
            if let Some((key, val)) = line.split_once('=') {
                let val = val.trim();
                assert!(!val.is_empty(), "Empty value for key: {}", key.trim());
            }
        }

        // Verify original structure preserved and mutation applied.
        assert!(output.contains("gaps_in = 10"));
        assert!(output.contains("border_size = 2"));
        assert!(output.contains("kb_layout = us"));
        assert!(!output.contains("kb_variant"));
        assert!(!output.contains("kb_model"));
        assert!(!output.contains("kb_options"));
        assert!(!output.contains("kb_rules"));
    }
}
