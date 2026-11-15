//! Authored and revised by local Engineer Gent, c2d7a7e0-8223-474d-8ccb-8caf0331b359.
//! Pure lexical filtering only; the descriptor adapter independently confines I/O.
//! Host acceptance includes independent Unicode, traversal and boundary tests.
pub fn is_safe_relative_path(path: &str) -> bool {
    // Check total UTF-8 byte length <= 1000
    if path.len() > 1000 {
        return false;
    }

    // Split path into components
    let components: Vec<&str> = path.split('/').collect();

    // Check at most 12 components
    if components.len() > 12 {
        return false;
    }

    // Check each component
    for component in &components {
        // Check for empty components
        if component.is_empty() {
            return false;
        }

        // Check if component starts with a dot
        if component.starts_with('.') {
            return false;
        }

        // Check for backslashes, colons, NUL, or Unicode control characters
        for ch in component.chars() {
            if ch == '\\' || ch == ':' || ch == '\0' || ch.is_control() {
                return false;
            }
        }

        // Check component length <= 120 UTF-8 bytes
        if component.len() > 120 {
            return false;
        }
    }

    // Reject absolute paths
    if path.starts_with('/') {
        return false;
    }

    // Reject dot and dot-dot traversal
    if components.contains(&"..") || components.contains(&".") {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_paths() {
        assert!(is_safe_relative_path("file.txt"));
        assert!(is_safe_relative_path("folder/file.txt"));
        assert!(is_safe_relative_path(
            "folder/subfolder/file with spaces.txt"
        ));
        assert!(is_safe_relative_path("a/b/c/d/e/f/g/h/i/j/k/l")); // 12 components max
    }

    #[test]
    fn test_invalid_paths() {
        assert!(!is_safe_relative_path("/absolute/path"));
        assert!(!is_safe_relative_path("")); // empty path
        assert!(!is_safe_relative_path(".")); // dot traversal
        assert!(!is_safe_relative_path("..")); // dot-dot traversal
        assert!(!is_safe_relative_path("folder/.."));
        assert!(!is_safe_relative_path("folder/."));
        assert!(!is_safe_relative_path("folder/./subfolder"));
        assert!(!is_safe_relative_path("folder/../subfolder"));
        assert!(!is_safe_relative_path("folder/\\")); // backslash
        assert!(!is_safe_relative_path("folder:")); // colon
        assert!(!is_safe_relative_path("folder\0")); // NUL
        assert!(!is_safe_relative_path("folder\u{0001}")); // control character
        assert!(!is_safe_relative_path("a/b/c/d/e/f/g/h/i/j/k/l/m")); // too many components
        assert!(!is_safe_relative_path(&"a".repeat(121))); // component too long
    }
}
