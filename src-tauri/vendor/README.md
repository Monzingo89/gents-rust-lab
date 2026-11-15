# Reviewed GTK 3 compatibility backports

The optional Tauri desktop dependency graph still uses GTK 3 / GLib 0.18 on Linux.
The browser website and Rust workspace do not link these crates. Updating Tauri
alone does not move its GTK dependency to GLib 0.20.

The following crates.io source packages are retained here with their original
MIT notices and original version identities:

- `glib` 0.18.5: apply the two-line upstream fix from
  [gtk-rs/gtk-rs-core#1343](https://github.com/gtk-rs/gtk-rs-core/pull/1343), commit
  `b5a4071e439bef2b5eea76c3aa25e5ae84839e34`. Its C out-pointer is mutable and passed
  as `&mut p`. This backports the fix for **RUSTSEC-2024-0429**.
- `glib-macros` 0.18.5 and `gtk3-macros` 0.18.2: replace the unmaintained
  `proc-macro-error` dependency with `proc-macro-error3` 3.1.1 (syn 2 compatibility feature; default syn 3 support disabled) and update its import
  namespace. Both packages use syn 2 already. Original macro names are unchanged.

`provenance.json` records SHA-256 for every original source file and each modified
file (including whitespace-only normalization in notices and one doc comment). `scripts/audit-desktop-dependencies.py` checks the exact retained file set and
hashes before allowing the single GLib version advisory exception. It rejects all
other advisories/warnings. The exception is necessary because cargo-audit matches
package versions, including this original version number, rather than inspecting
the patched source. It is not a claim that unmodified GLib 0.18.5 is safe.

The relevant GLib iterator tests must run with optimization in CI. The independent
macOS desktop check and macro compilation are additional compatibility checks;
they do not establish a Linux GUI release or general absence of vulnerabilities.
Do not change these sources without updating provenance and reviewing the patch.
Remove the backports and exception when the desktop dependency graph moves to
maintained compatible upstream packages.
