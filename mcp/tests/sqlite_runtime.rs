#[test]
fn native_database_runtime_contains_the_advisory_fix() {
    // RUSTSEC-2026-0459 concerns the linked SQLite, not merely a Rust package label.
    assert!(rusqlite::version_number() >= 3_054_000);
    let db = rusqlite::Connection::open_in_memory().unwrap();
    let observed: String = db
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .unwrap();
    assert_eq!(observed, rusqlite::version());
    println!("Patched native SQLite runtime: {observed}");
}
