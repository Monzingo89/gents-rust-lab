# 01 — Start from a clean Mac and VS Code

## 1. Install the tools

1. Install [VS Code for macOS](https://code.visualstudio.com/docs/setup/mac). Open it once. In the Command Palette, run **Shell Command: Install 'code' command in PATH**.
2. Install Apple's full Xcode from the Mac App Store for iOS work. Launch Xcode, accept the license and install its requested platform components. The command line tools alone suffice for the initial backend exercise:

```bash
xcode-select --install
```

3. Install Rust using the official [rustup](https://rustup.rs/) installer. Read the installer before executing it if desired:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/gents-rustup.sh
sh /tmp/gents-rustup.sh -y
source "$HOME/.cargo/env"
rustc --version
cargo --version
rustup component add rustfmt clippy
```

4. Install current Node.js LTS from [nodejs.org](https://nodejs.org/en/download). The web bundler requires a supported modern Node version; use Node 22.12+ or a newer supported LTS. Verify:

```bash
node --version
npm --version
```

5. Install [Azure CLI](https://learn.microsoft.com/en-us/cli/azure/install-azure-cli-macos) when you reach the Azure stage. If Homebrew is installed:

```bash
brew update
brew install azure-cli
az version
```

You do not need Docker, Azure, Hugging Face, OpenRouter or an Apple Developer membership for the mock backend.

## 2. Open the project

Unzip the download, move `gents-rust-lab` to your development directory, then open that actual folder:

```bash
cd /path/to/gents-rust-lab
code .
```

Accept the workspace extension recommendations: Rust Analyzer and Even Better TOML. Open **Terminal → New Terminal**. Every command below runs inside this project root.

## 3. Compile and test

```bash
cargo test --workspace --locked
cargo run -p gents-harness --locked
```

The first build downloads crates and takes longer. Leave the process running. Open `http://127.0.0.1:8787` in your browser. Click **Connect**, then **Start task**.

The default `config/mock.json` defines two agents. Each has a simulated planner and worker. The judge-enabled default simulates two rounds and thirteen calls, including both judges; chapter 12 explains the route → work → review → repair cycle. Use `CONFIG=config/fixed.mock.json cargo run -p gents-harness --locked` for the original six-call fixed-team demonstration. Token counts are unknown because no tokenizer or provider was called; provider cost is zero. The mock output intentionally does not answer the vendor question.

Open an activity entry to inspect a response. Use **Export event JSON** to retain the run outside the backend journal.

## 4. Run it as a native window

Open a second VS Code terminal without stopping Rust:

```bash
npm ci
npm run tauri dev
```

Tauri starts the web dev server and opens the desktop shell. Click **Connect** inside that window. The backend remains a separate process in this first stage. Closing the window does not stop the backend; Ctrl+C in its terminal does.

## 5. Create your own git history

```bash
git init
git add .
git commit -m "Start Gents Rust harness lab"
```

Never commit tokens or `.env` files. `config/local.json` is ignored so local endpoint configuration stays separate from the reference examples.

**Checkpoint:** browser mock run works; you can identify each planner and worker, and the native shell opens on your Mac. If Tauri fails while the browser works, inspect the platform prerequisites before changing harness code.
