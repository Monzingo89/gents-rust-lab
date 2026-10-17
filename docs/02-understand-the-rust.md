# 02 — Understand and change the Rust

This chapter traces fixed mode (`CONFIG=config/fixed.mock.json`). Judge mode is implemented in `judges.rs` and explained in chapter 12. Open these files in VS Code in this order. Complete source is supplied; you do not need to paste fragments together.

## 1. Follow the model boundary

In `provider.rs`, locate `LanguageModel`. Its `complete` method receives a model configuration, instructions, input text and an output cap. It returns `Reply`: text, normalized usage, latency and response ID.

`Gateway` implements the trait. The mock branch simulates a response. The live branch sends a JSON chat-completions request to the configured HTTPS endpoint. It uses one adapter for compatible text APIs, with an OpenRouter-specific usage option. Other API shapes, tool calls and multimodal messages require additional adapters and capability checks.

This is model agnosticism: vendor changes happen at the boundary; the run algorithm does not need a vendor SDK. It is not a promise that every model supports every request field.

## 2. Locate the actual harness

In `main.rs`:

| Symbol | Responsibility |
|---|---|
| `Config::validate` | Reject unbounded or invalid lab configuration |
| `start` | Authorize, validate input and create a run |
| `execute` | Coordinate agent handoffs and planner/worker lifecycle |
| `parse_plan` | Convert untrusted model text into a bounded typed plan |
| `call` | Emit call events and invoke the model boundary |
| `emit` | Append the observable event stream |
| `snapshot` | Return run state only to its owner |
| `cancel` | Request cancellation before the next model call |

Read `execute` line by line. `previous` is the last agent's final output. `input` combines task, assignment and predecessor output. The planner returns `Plan`. The worker iterates its steps, receiving `draft` from the previous step. The final draft becomes `previous` for the next agent.

The planner and worker are roles, not separate OS processes. The UI gives each role its own sprite so you can see the distinction.

## 3. Change assignments

Stop the server with Ctrl+C. Copy the mock configuration:

```bash
cp config/fixed.mock.json config/local.json
```

Edit `config/local.json`: make Atlas the researcher and Vera the reviewer. With mock models this changes the input, not actual intelligence. Restart:

```bash
CONFIG=config/local.json cargo run -p gents-harness --locked
```

Add a third agent by duplicating an agent object and changing its ID to a unique name. The UI creates two more sprites after reconnecting. Remove agents to test one pair. The architecture accepts a list; the lab deliberately enforces 1..8 agents to bound activity.

## 4. Inspect a failed plan

Temporarily change the mock planner's JSON in `provider.rs` to produce an empty `steps` array. Run the task and inspect the failure. Revert the change, then run:

```bash
cargo test --workspace --locked
```

This illustrates why the harness validates model output instead of trusting it as executable instructions.

## 5. Understand limits and cancellation

At most two runs are active in this single-process lab. A run has at most `agents × (1 + max_steps)` calls. The maximum configuration is 8 × 4 = 32 calls, and each requests at most 4,096 completion tokens. Input tokens, provider-specific reasoning policies, partial failures and hosted compute still affect cost. These are fixed-mode request limits, not a hard billing guarantee. Judge mode uses a separate run-wide `max_calls` cap including every router, evaluator, planner and worker call.

Cancel lets an active call finish and record available usage; the next call does not begin. Closing the app or losing the network does not cancel a server run. A timeout or disconnect can still incur a provider charge. There are no automatic retries.

**Checkpoint:** explain which function is the harness, where a planner is called, where a worker is called, and how adding an agent changes the run.
