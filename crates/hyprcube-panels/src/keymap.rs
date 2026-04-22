/// Convert a panel field's section context and key to a Hyprland IPC keyword
/// path.
///
/// Hyprland's `/keyword` command uses colon-separated paths:
/// - `"general"` + `"border_size"` → `"general:border_size"`
/// - `"input:touchpad"` + `"natural_scroll"` → `"input:touchpad:natural_scroll"`
pub fn panel_key_to_ipc_keyword(section: &str, key: &str) -> String {
    format!("{section}:{key}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_section() {
        assert_eq!(
            panel_key_to_ipc_keyword("general", "border_size"),
            "general:border_size"
        );
    }

    #[test]
    fn nested_section() {
        assert_eq!(
            panel_key_to_ipc_keyword("input:touchpad", "natural_scroll"),
            "input:touchpad:natural_scroll"
        );
    }

    #[test]
    fn decoration_section() {
        assert_eq!(
            panel_key_to_ipc_keyword("decoration", "rounding"),
            "decoration:rounding"
        );
    }
}
