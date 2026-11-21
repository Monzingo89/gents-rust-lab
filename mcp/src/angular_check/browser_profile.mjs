import path from 'node:path';
const literal = value => JSON.stringify(value);
export function browserProfile(browser, profileDir, port) {
// Chromium's own sandbox_init cannot nest under this outer sandbox on the
// supported host. --no-sandbox is fixed host code, and NEVER removes this
// outer profile. Every browser process inherits the same filesystem/egress
// boundary. No alternate executable, flag, profile or endpoint is accepted.
return `(version 1)(deny default)
(allow sysctl-read)(deny sysctl-read (sysctl-name-prefix "kern.procargs"))
(allow file-read-metadata)
(allow file-read* (literal "/") (subpath "/System/Library") (subpath "/usr/lib") (subpath "/usr/share/icu") (subpath "/private/var/db/timezone") (subpath "/Library/Fonts") (subpath "/private/var/db/dyld") (literal "/private/etc/passwd") (subpath ${literal(path.dirname(browser))}) (subpath ${literal(profileDir)})(literal "/dev/null")(literal "/dev/random")(literal "/dev/urandom"))
(allow file-write* (subpath ${literal(profileDir)})(literal "/dev/null"))
(allow mach-bootstrap)(allow mach-register (local-name-prefix ""))
(allow mach-register (global-name-prefix "org.chromium."))(allow mach-lookup (global-name-prefix "org.chromium."))
(allow mach-lookup (global-name "com.apple.bsd.dirhelper"))
(allow process-fork)(allow process-exec (subpath ${literal(path.dirname(browser))}))
(allow signal (target same-sandbox))
(allow network-outbound (remote tcp "localhost:${port}"))
(allow system-socket (socket-domain AF_UNIX))
(allow network-bind (local unix-socket (subpath ${literal(profileDir)})))
(allow network-outbound (remote unix-socket (subpath ${literal(profileDir)})))
(deny network-bind (local ip "*:*"))
(deny process-info*)(allow process-info* (target self))`;
}
