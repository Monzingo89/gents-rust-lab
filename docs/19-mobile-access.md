# Same-Wi-Fi mobile control

From this repository on the laptop:

```sh
npm run company:mobile
```

Leave that terminal running. The command reuses an existing local company on
127.0.0.1:8788, or starts the normal company supervisor if needed. It does not
create a second set of agents or move inference to the phone/cloud.

1. The laptop opens `http://127.0.0.1:8790`. This setup view is laptop-only.
2. Connect the phone to the same trusted Wi-Fi, not cellular or isolated guest Wi-Fi.
3. Open the exact `https://<laptop-wifi-ip>:8789` address shown there on the phone.
4. The first visit warns about a self-signed local certificate. Confirm the exact
   Wi-Fi address against the laptop setup page before proceeding via the browser's
   advanced/details option. This exception is only for your own laptop address.
   A SHA-256 certificate fingerprint is shown on the laptop for comparison where
   the browser offers certificate details. If a managed phone prohibits local
   certificate exceptions, use an administrator-trusted certificate solution;
   do not disable browser security globally.
5. Enter the case-sensitive pairing code from the laptop setup view. The code is
   not printed to the terminal, put in a URL, or sent to an outside service.
6. Select **Ask**, describe the task, approve it and submit. The laptop executes it.

The phone can see the office, public activity, work-log entries, token counts,
saved runs, training progress, CEO information and questions. Tap a Gent for its
dossier. Use the office zoom slider and swipe across the scene for larger agents.
The bottom navigation jumps between Office, Ask, Work log and Models. Work log
exports are available. Private model reasoning is not exposed; public activity
and evidence are. No new model or email API keys are needed for mobile access.

An idle view follows newly started company runs, including runs started on the
other device. Selecting a saved run stops auto-follow until **Watch company work**
is selected. Reloading/reopening the page restores or discovers the current run;
resuming a phone browser refreshes company state. Closing a browser does not
cancel a task. Task acceptance rules and admin review are unchanged.

## Laptop availability

Keep the laptop plugged in, **lid open**, connected to Wi-Fi, and the company
terminal running. On macOS the launcher requests idle-sleep prevention with
`caffeinate`; this does not promise closed-lid operation or wake a powered-off Mac.
Allow Node.js incoming connections if the macOS firewall asks. The phone cannot
start a stopped/offline laptop, repair its network, or bypass Wi-Fi client isolation.

The Wi-Fi address can change after reconnecting or moving networks. Restart the
mobile command to select the new address/certificate and show a new code. If the
wrong interface is selected, choose a private address belonging to this laptop:

```sh
GENTS_MOBILE_HOST=192.168.1.97 npm run company:mobile
```

Replace the example with the actual Wi-Fi address. No wildcard/public binding is
accepted. A Wi-Fi change while running requires a mobile-access restart; this
does not require canceling an already-running company.

## Security and stopping

- The agent harness remains loopback-only. A separate HTTPS gateway binds only
  the selected private IPv4 address on 8789. There is no router forwarding,
  internet tunnel, public hosting, Tailscale requirement or cellular access.
- Pairing uses a random per-process code and rate limits. Sessions use Secure,
  HttpOnly, SameSite=Strict cookies and expire after seven days or gateway restart.
  State-changing requests require the exact same-origin HTTPS address.
- **Disconnect this phone** revokes its session without stopping laptop work.
- Ctrl-C stops the mobile gateway and revokes all phone sessions. If this command
  started the company it also stops its own company supervisor; if it reused a
  company already running in another terminal, that company is left running.
- Certificates/private keys are generated locally in ignored `data/mobile/`;
  the private key has mode 0600. Do not share this directory or pairing code.
- LAN isolation means private-network-only, not a claim of physical Wi-Fi
  proximity: other routed private networks can reach the selected interface if
  your network permits it, but pairing and HTTPS still apply. Use a trusted LAN.

Desktop/mobile automated checks use simulated jobs and isolated fixture servers;
they are not evidence that a particular physical phone has connected or that a
real model task/email completed.
