# 01a — Optional hand-coding route from an empty directory

Chapter 01 is the fastest way to run the supplied project. Use this route if you want to create every source file yourself instead. `START-HERE.html` contains the complete source appendix with copyable code blocks. The ZIP also contains those same files in their final locations.

## 1. Create your empty project

Install the tools in chapter 01, then:

```bash
mkdir -p ~/Developer/gents-rust-from-scratch
cd ~/Developer/gents-rust-from-scratch
mkdir -p crates/harness/src config web infra docs scripts
mkdir -p src-tauri/src src-tauri/capabilities .vscode
code .
```

In VS Code, use **New File** and enter each path from the source appendix. Paste its entire block, then save. Do not mix partial files from different iterations.

## 2. Create the Rust backend first

Create these files in order:

1. Root `Cargo.toml`: defines the workspace containing the harness crate.
2. `crates/harness/Cargo.toml`: defines HTTP, async, serialization, auth and provider dependencies.
3. `crates/harness/src/provider.rs`: defines the provider trait, mock behavior and live adapter.
4. `crates/harness/src/auth.rs`: defines the API token-validation boundary.
5. `crates/harness/src/store.rs` and `crates/harness/src/acceptance.rs`: journal and deterministic checks.
6. `crates/harness/src/judges.rs`: routing contracts, validators and repair loop.
7. `crates/harness/src/main.rs`: defines the actual harness and HTTP routes.
8. `config/mock.json`: defines the two planner/worker pairs.

Generate a dependency lockfile and run the unit tests:

```bash
cargo generate-lockfile
cargo test --workspace
```

The backend can compile before the UI exists. Test the API after launching it:

```bash
cargo run -p gents-harness
```

In a second terminal:

```bash
curl http://127.0.0.1:8787/health
curl http://127.0.0.1:8787/api/config
curl -X POST http://127.0.0.1:8787/api/runs \
  -H 'Content-Type: application/json' \
  -d '{"task":"Compare Vendor A at $20 for 2 users and Vendor B at $35 for 5 users, for our team of 4.","consent":false}'
```

Copy the returned run UUID and inspect it:

```bash
curl http://127.0.0.1:8787/api/runs/PASTE_RUN_UUID
```

You have a functioning harness without a GUI. Watch the event array grow and inspect terminal status. This proves the harness is the Rust lifecycle logic, not the office animation.

## 3. Add the office

Create `web/index.html`, `web/style.css` and `web/app.js` from the appendix. Open `http://127.0.0.1:8787`, connect and run a task. Static files are served by Rust; no Node process is required for this browser exercise.

Find the event names in `app.js` and compare them with `emit` calls in Rust. Room movement consumes these reported states. Modify a room color, refresh, and confirm the backend behavior is unaffected.

## 4. Add the native Mac shell

Create `package.json`, the files under `src-tauri/`, and its capability file. Run:

```bash
npm install
npm run tauri dev
```

Keep the Rust harness running in the original terminal. You now have the same office in a native window. Node builds the frontend; it does not run the agent orchestration.

## 5. Add the remaining stages

Create `.gitignore`, `.env.example`, VS Code configuration, live model example, smoke script, Dockerfile, `.dockerignore` and Bicep files. Then continue with chapters 02 through 09. Do not add cloud credentials until the local mock run is understood.

When reconstructing by hand, the dependency resolver may select newer compatible packages than the supplied ZIP's lockfiles. To reproduce the verified dependency set, copy the supplied root `Cargo.lock` and `package-lock.json` before using `--locked` / `npm ci`. The native Tauri lockfile belongs in `src-tauri/Cargo.lock`.

**Checkpoint:** you can build the harness from empty folders, operate it with curl, add the GUI, and add the desktop shell as separate steps.
