//! Authored by local Engineer Gent; initial run d07df27a-0449-4b9b-9481-4bd0fe602cc7.
//! Local 30B revision 380a333f-3878-49b7-986d-b7196f16ae0e resolved strict lint feedback.
//! Promoted after source review and independent edge/property tests; no I/O.
pub fn packet_budget(context_limit: usize, reserved: usize, requested: usize) -> usize {
    // Handle potential overflow in addition before subtraction
    // If reserved >= context_limit, remaining is 0 or negative (clamped to 0)
    // We use saturating_sub to avoid panic, but logic dictates if reserved >= limit, result is 0.

    // Calculate remaining capacity safely
    let remaining = context_limit.saturating_sub(reserved);

    // The allowed amount cannot exceed the remaining capacity
    // and cannot exceed the requested amount.
    // We need min(requested, remaining).
    // To avoid overflow in min calculation if both are usize::MAX (though min handles it correctly in Rust),
    // we can use a conditional or saturating logic.
    // Since requested and remaining are both usize, min is safe in Rust.

    requested.min(remaining)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_calculation() {
        assert_eq!(packet_budget(100, 20, 50), 50);
    }

    #[test]
    fn test_requested_exceeds_remaining() {
        assert_eq!(packet_budget(100, 20, 150), 80); // min(150, 80) = 80
    }

    #[test]
    fn test_reserved_exhausts_context() {
        assert_eq!(packet_budget(100, 100, 50), 0);
    }

    #[test]
    fn test_reserved_exceeds_context() {
        assert_eq!(packet_budget(100, 150, 50), 0);
    }

    #[test]
    fn test_zero_context_limit() {
        assert_eq!(packet_budget(0, 0, 50), 0);
    }

    #[test]
    fn test_zero_requested() {
        assert_eq!(packet_budget(100, 0, 0), 0);
    }

    #[test]
    fn test_usize_max_safe() {
        // Ensure no panic or overflow with usize::MAX
        let max_val = usize::MAX;
        // If context is max, reserved is 0, requested is 0 -> 0
        assert_eq!(packet_budget(max_val, 0, 0), 0);
        // If context is max, reserved is 1, requested is 0 -> 0
        assert_eq!(packet_budget(max_val, 1, 0), 0);
        // If context is max, reserved is 0, requested is 1 -> 1
        assert_eq!(packet_budget(max_val, 0, 1), 1);
        // Edge case: reserved is max, context is max -> 0
        assert_eq!(packet_budget(max_val, max_val, 1), 0);
    }
}
