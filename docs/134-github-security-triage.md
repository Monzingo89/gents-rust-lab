# GitHub security triage — November 14, 2026

Scope: GitHub Dependabot, secret scanning and CodeQL for this repository. This is
an advisory snapshot, not a claim that no vulnerabilities exist.

Dependabot alerts 1–3 are fixed on GitHub: jsonwebtoken was updated and the GLib
iterator fix was retained as a hash-verified backport in the Linux desktop graph.
Secret scanning reported no open alerts during this review.

## CodeQL Rust findings

The retained private SARIF from analysis 2169982623 was inspected, including its
source-to-sink paths. Findings 3–8 are false positives, rather than vulnerabilities
silenced through changes to security checks or broad path exclusions:

- **3 / cleartext logging:** the reported source is `g_uuid_string_is_valid`,
  whose return value is a gboolean validity predicate, not the UUID string. The
  trace runs through generic `from_glib` into `LogLevel::from_glib(u32)` and its
  unknown-level panic. A boolean neither carries the string nor supplies a valid
  log level. No UUID or credential is logged by this path.
- **4, 6, 7 / invalid pointer:** the source is `console_charset`'s initially null
  `const char *`, filled by `g_get_console_charset(&mut charset)`. The actual
  return type selects `GString` conversion. The trace merges generic trait
  implementations into `ObjectRef`, a boxed test type, or `Value`; these have
  incompatible pointer types and are not the instantiated conversion. C writes
  the out-pointer before the string conversion.
- **5, 8 / invalid pointer:** `file_set_contents` initializes a `GError *` to null
  as required by GLib. The C call fills it on failure; the wrapper tests null
  before selecting `Error::from_glib_full`. The reported `ObjectRef`/`Value`
  conversion is an unrelated generic-trait instantiation. No null pointer reaches
  those reported dereferences from this call.
- **9 / pointer slice truncation:** retain the owner's existing move-out change:
  read the owned item, clear the removed slot, then drop the moved item. A new
  optimized test uses weak object references to verify exactly the removed
  objects are released, including no-op and repeated-empty truncation. The
  reviewed source hash is now included in provenance; the preceding GitHub
  auto-fix omitted it and caused the dependency gate to fail.

The broader optimized run also exposed five upstream test-only out-of-bounds
`get_unchecked(4)` calls on four-element slices. Those terminator assertions now
read through the allocation pointer, as required by the null-terminated backing
storage contract, and retain a reviewed source hash.

All GLib library tests now run in the optimized CI backport job, covering both
the iterator advisory and pointer-slice regression. There is no blanket vendor
exclusion or blanket acceptance of unsafe code. New findings still need review.

Primary contracts: [console charset](https://docs.gtk.org/glib/func.get_console_charset.html),
[file contents and GError](https://docs.gtk.org/glib/func.file_set_contents.html),
[UUID validity predicate](https://docs.gtk.org/glib/func.uuid_string_is_valid.html).

Only the optional Linux desktop links vendored GTK/GLib; the hosted static website
and Rust harness do not. Source promotion, GitHub rescanning, CI and website
artifact verification are separate release checks.
