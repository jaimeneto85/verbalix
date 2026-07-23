pub fn is_main_window(label: &str) -> bool {
    label == "main"
}

#[cfg(test)]
mod tests {
    use super::is_main_window;

    #[test]
    fn only_the_main_window_uses_hide_on_close_lifecycle() {
        assert!(is_main_window("main"));
        assert!(!is_main_window("toolbar"));
        assert!(!is_main_window("note"));
    }
}
