//! Independent host acceptance suite, not the local author's own tests.
use gents_local_mcp::packet_budget::packet_budget;
#[test]
fn exhaustive_edges_and_seeded_inputs_match_a_wider_integer_oracle() {
    let edges = [
        0,
        1,
        2,
        255,
        256,
        1024,
        16384,
        usize::MAX / 2,
        usize::MAX - 1,
        usize::MAX,
    ];
    let verify = |limit: usize, reserved: usize, requested: usize| {
        let actual = packet_budget(limit, reserved, requested);
        let remaining = (limit as u128).saturating_sub(reserved as u128);
        assert_eq!(actual as u128, (requested as u128).min(remaining));
        assert!(actual <= requested);
        assert!(actual as u128 + reserved.min(limit) as u128 <= limit as u128);
    };
    for limit in edges {
        for reserved in edges {
            for requested in edges {
                verify(limit, reserved, requested);
            }
        }
    }
    let mut seed = 0x17f31de845ab193du64;
    for _ in 0..10000 {
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            seed as usize
        };
        verify(next(), next(), next());
    }
}
