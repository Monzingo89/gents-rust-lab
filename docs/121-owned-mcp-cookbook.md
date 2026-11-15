# Owned native MCP cookbook

This covers all 15 tools in the local native implementation catalog captured on 2026-11-13. It is a source-derived recipe/reference, not a list of an agent's grants, proof that every task works, a security attestation or a migration claim for legacy Python tools. Tool calls target $0 provider fees; models, hardware and electricity are separate.

## Before calling any tool

Confirm the task/definition of done, review the exact tool/source/API identity, freeze the ordinary intake, approve its digest and install current actor/phase permissions. The trusted host binds company, owner, run, actor and phase to the process. Model JSON cannot replace those values or acquire authority.

Discover implementation metadata in the B1 MCP room or `GET /api/mcp/catalog`. An initialized MCP `tools/list` response is instead filtered to the current actor, phase and policy. Do not launch an ad hoc privileged server or copy a bearer token into a prompt/log.

For the existing local launcher, choose the transport without changing domain tool code:

```sh
GENTS_MCP_TRANSPORT=stdio npm run company
# Or choose the loopback Streamable HTTP/SSE adapter:
GENTS_MCP_TRANSPORT=sse npm run company
```

Keep the terminal running. The launcher refuses an incompatible already-running mode. Current HTTP/SSE is one private child/session and one `/mcp` endpoint; it is not a public server or the deprecated paired SSE/message transport. See [transport boundaries](39-mcp-basement-and-local-ceo.md). A mode switch never broadens permissions or migrates legacy Python tools.

Native receipts record source/action/result identities and current policy. A successful call is not final task acceptance. Rejected/uncertain effects need reconciliation; never replay a write merely because a response was lost. Use owner-frozen artifact/dependency contracts for multi-tool work, not prose or manually substituted upstream answers.

## Choose the smallest sufficient capability

| Native name | Recipe | Implementation phases |
| --- | --- | --- |
| [audio_generate](#audio-generate) | Speak a short script locally | worker |
| [audio_inspect](#audio-inspect) | Check the structure of a small WAV | planner, worker |
| [files](#files) | Read, write or list a task workspace | planner, worker |
| [host_resources](#host-resources) | Check local capacity before choosing a model | planner, worker |
| [image_generate](#image-generate) | Create a small image locally | worker |
| [integer_arithmetic](#integer-arithmetic) | Calculate exact integer rows | planner, worker |
| [memory_append](#memory-append) | Keep an attributable advisory or experiment | planner, worker |
| [memory_search](#memory-search) | Retrieve currently allowed memory | planner, worker |
| [owned_rust_call](#owned-rust-call) | Use an admitted pure function | worker |
| [rust_verify](#rust-verify) | Check a quarantined pure Rust candidate | worker |
| [skill_list](#skill-list) | Discover assigned instructions | planner, worker |
| [skill_read](#skill-read) | Read one assigned instruction set | planner, worker |
| [table_aggregate](#table-aggregate) | Group exact integer data | planner, worker |
| [table_join](#table-join) | Combine two bounded flat tables | planner, worker |
| [table_read_json](#table-read-json) | Import a complete bounded JSON table | planner, worker |

## audio generate

**Recipe: Speak a short script locally.** Freeze an approved confined output_path and grant audio_generate to the assigned worker. Supply the exact text and supported local voice. The installed pinned Piper model creates a WAV without network or downloads. Inspect the actual receipt and WAV properties; the generic voice is not cloned, and speech quality still needs listening. Existing destinations and frozen inputs cannot be overwritten.

Implementation: [mcp/src/functions/media_generate.rs](../mcp/src/functions/media_generate.rs). Descriptor snapshot SHA-256: `d7fcb0866dac99e104507bcc1b9a956f8c97829c15da7fcaf37de82a3550fb4b`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "audio_generate",
  "description": "Generate one local mono PCM16 WAV narration from <=600 characters using the pinned Piper/LJ Speech CPU adapter. Fixed generic English voice; no cloning, music, transcription or identity claim. Requires current worker grant and approved active job. Offline isolated process, <=8 MiB, new confined output path only; no overwrite, downloads, paid fallback or automatic semantic acceptance. Current isolation supports macOS only.",
  "source": "mcp/src/functions/media_generate.rs",
  "phases": [
    "worker"
  ],
  "read_only": false,
  "deterministic": false,
  "external_effects": true,
  "runtime_requirements": {
    "call_timeout_seconds": 180,
    "installed_rustc": false
  },
  "data_input_fields": [],
  "file_input_fields": [],
  "provider_cost_usd": 0,
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "path": {
        "maxLength": 240,
        "minLength": 1,
        "type": "string"
      },
      "text": {
        "maxLength": 600,
        "minLength": 1,
        "type": "string"
      }
    },
    "required": [
      "text",
      "path"
    ],
    "type": "object"
  },
  "resultSchema": {
    "additionalProperties": false,
    "properties": {
      "artifact": {
        "additionalProperties": false,
        "properties": {
          "bytes": {
            "maximum": 8388608,
            "minimum": 1,
            "type": "integer"
          },
          "created_new": {
            "const": true
          },
          "path": {
            "type": "string"
          },
          "retry_rule": {
            "type": "string"
          },
          "sha256": {
            "pattern": "^[a-f0-9]{64}$",
            "type": "string"
          }
        },
        "required": [
          "path",
          "bytes",
          "sha256",
          "created_new",
          "retry_rule"
        ],
        "type": "object"
      },
      "audio": {
        "type": "object"
      },
      "engine": {
        "const": "piper_ljspeech_cpu"
      },
      "mime_type": {
        "const": "audio/wav"
      },
      "model_manifest_sha256": {
        "pattern": "^[a-f0-9]{64}$",
        "type": "string"
      },
      "provider_cost_usd": {
        "const": 0
      },
      "request_sha256": {
        "pattern": "^[a-f0-9]{64}$",
        "type": "string"
      },
      "semantic_acceptance": {
        "const": false
      }
    },
    "required": [
      "artifact",
      "mime_type",
      "engine",
      "request_sha256",
      "model_manifest_sha256",
      "provider_cost_usd",
      "semantic_acceptance",
      "audio"
    ],
    "type": "object"
  }
}
```

</details>

## audio inspect

**Recipe: Check the structure of a small WAV.** Freeze its confined path and complete file hash, then inspect PCM16 frame/rate/channel statistics. Inspect the native receipt and exact source hash. Do not infer speech, content, health, clipping or transcription from these statistics.

Implementation: [mcp/src/functions/audio_inspect.rs](../mcp/src/functions/audio_inspect.rs). Descriptor snapshot SHA-256: `726dacb654f69c663f0644f220cbd5854a1737af9ff6c7d37aca8b59927a9c11`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "audio_inspect",
  "description": "Inspect one complete hash-pinned confined .wav file (<=16 MiB), little-endian mono/stereo integer PCM16 only. Returns source provenance, exact frame/rate duration, per-channel extrema, peak absolute sample, sum, zero/rail counts and adjacent nonzero sign changes. Rejects malformed/ambiguous headers, partial frames and unsupported codecs. Ignores unknown metadata payloads without echoing them. Read-only current actor grant required. No playback, transcription, speaker/content/health inference, network, executable codecs, downloads or source modification. Rail hits alone do not prove clipping.",
  "source": "mcp/src/functions/audio_inspect.rs",
  "phases": [
    "planner",
    "worker"
  ],
  "read_only": true,
  "deterministic": false,
  "external_effects": false,
  "runtime_requirements": {
    "call_timeout_seconds": 10,
    "installed_rustc": false
  },
  "data_input_fields": [],
  "file_input_fields": [
    "path"
  ],
  "provider_cost_usd": 0,
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "expected_sha256": {
        "maxLength": 64,
        "minLength": 64,
        "type": "string"
      },
      "path": {
        "maxLength": 120,
        "minLength": 1,
        "type": "string"
      }
    },
    "required": [
      "path",
      "expected_sha256"
    ],
    "type": "object"
  },
  "resultSchema": {
    "additionalProperties": false,
    "properties": {
      "all_samples_zero": {
        "type": "boolean"
      },
      "bits_per_sample": {
        "const": 16
      },
      "channel_statistics": {
        "items": {
          "additionalProperties": false,
          "properties": {
            "adjacent_sign_changes": {
              "maximum": 8388608,
              "minimum": 0,
              "type": "integer"
            },
            "channel": {
              "maximum": 1,
              "minimum": 0,
              "type": "integer"
            },
            "maximum": {
              "oneOf": [
                {
                  "maximum": 32767,
                  "minimum": -32768,
                  "type": "integer"
                },
                {
                  "type": "null"
                }
              ]
            },
            "minimum": {
              "oneOf": [
                {
                  "maximum": 32767,
                  "minimum": -32768,
                  "type": "integer"
                },
                {
                  "type": "null"
                }
              ]
            },
            "peak_abs": {
              "maximum": 32768,
              "minimum": 0,
              "type": "integer"
            },
            "rail_samples": {
              "maximum": 8388608,
              "minimum": 0,
              "type": "integer"
            },
            "sample_sum": {
              "maximum": 274877906944,
              "minimum": -274877906944,
              "type": "integer"
            },
            "zero_samples": {
              "maximum": 8388608,
              "minimum": 0,
              "type": "integer"
            }
          },
          "required": [
            "channel",
            "minimum",
            "maximum",
            "peak_abs",
            "sample_sum",
            "zero_samples",
            "rail_samples",
            "adjacent_sign_changes"
          ],
          "type": "object"
        },
        "maxItems": 2,
        "minItems": 1,
        "type": "array"
      },
      "channels": {
        "maximum": 2,
        "minimum": 1,
        "type": "integer"
      },
      "duration_seconds": {
        "additionalProperties": false,
        "properties": {
          "denominator": {
            "maximum": 4294967295,
            "minimum": 1,
            "type": "integer"
          },
          "numerator": {
            "maximum": 8388608,
            "minimum": 0,
            "type": "integer"
          }
        },
        "required": [
          "numerator",
          "denominator"
        ],
        "type": "object"
      },
      "format": {
        "const": "wav_pcm16_le"
      },
      "frame_count": {
        "maximum": 8388608,
        "minimum": 0,
        "type": "integer"
      },
      "measurement_scope": {
        "type": "string"
      },
      "provider_cost_usd": {
        "const": 0
      },
      "sample_rate_hz": {
        "maximum": 4294967295,
        "minimum": 1,
        "type": "integer"
      },
      "source_bytes": {
        "maximum": 16777216,
        "minimum": 0,
        "type": "integer"
      },
      "source_path": {
        "type": "string"
      },
      "source_sha256": {
        "maxLength": 64,
        "minLength": 64,
        "type": "string"
      }
    },
    "required": [
      "source_path",
      "source_sha256",
      "source_bytes",
      "format",
      "channels",
      "sample_rate_hz",
      "bits_per_sample",
      "frame_count",
      "duration_seconds",
      "channel_statistics",
      "all_samples_zero",
      "measurement_scope",
      "provider_cost_usd"
    ],
    "type": "object"
  }
}
```

</details>

## files

**Recipe: Read, write or list a task workspace.** Use a specific operation grant. Read bounded pages and check whole-file hashes when completeness matters. A worker may atomically replace authorized text; a planner may not write. For a lost write response, reconcile the canonical receipt and actual file before any retry.

Implementation: [mcp/src/functions/files.rs](../mcp/src/functions/files.rs). Descriptor snapshot SHA-256: `4ea609e085febe83f0bb21b964a61f43e71f539482c372150d7bc56ba676ba83`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "files",
  "description": "Governed run-workspace files: read bounded UTF-8 pages, atomically replace bounded text, or list bounded safe entries. Optional write expected_sha256 requires read permission and an existing matching file hash before replacement; MCP writers are serialized. Frozen owner inputs cannot be overwritten. No symlinks, secret files, subprocesses or arbitrary paths. Planner phase cannot write.",
  "source": "mcp/src/functions/files.rs",
  "phases": [
    "planner",
    "worker"
  ],
  "read_only": false,
  "deterministic": false,
  "external_effects": true,
  "runtime_requirements": {
    "call_timeout_seconds": 10,
    "installed_rustc": false
  },
  "data_input_fields": [],
  "file_input_fields": [],
  "provider_cost_usd": 0,
  "inputSchema": {
    "oneOf": [
      {
        "additionalProperties": false,
        "properties": {
          "op": {
            "const": "list"
          },
          "path": {
            "maxLength": 2000,
            "type": "string"
          }
        },
        "required": [
          "op",
          "path"
        ],
        "type": "object"
      },
      {
        "additionalProperties": false,
        "properties": {
          "max_bytes": {
            "maximum": 8192,
            "minimum": 1,
            "type": "integer"
          },
          "offset_bytes": {
            "minimum": 0,
            "type": "integer"
          },
          "op": {
            "const": "read"
          },
          "path": {
            "maxLength": 2000,
            "minLength": 1,
            "type": "string"
          }
        },
        "required": [
          "op",
          "path"
        ],
        "type": "object"
      },
      {
        "additionalProperties": false,
        "properties": {
          "content": {
            "maxLength": 32768,
            "type": "string"
          },
          "expected_sha256": {
            "maxLength": 64,
            "minLength": 64,
            "pattern": "^[0-9a-f]{64}$",
            "type": "string"
          },
          "op": {
            "const": "write"
          },
          "path": {
            "maxLength": 2000,
            "minLength": 1,
            "type": "string"
          }
        },
        "required": [
          "op",
          "path",
          "content"
        ],
        "type": "object"
      }
    ],
    "type": "object"
  },
  "resultSchema": null
}
```

</details>

## host resources

**Recipe: Check local capacity before choosing a model.** Read the host estimate and any observed Linux process limit. Keep unobserved GPU memory unknown, preserve a context/runtime allowance and fail capacity admission when required process limits are unknown. This does not download or select a model.

Implementation: [mcp/src/functions/host_resources.rs](../mcp/src/functions/host_resources.rs). Descriptor snapshot SHA-256: `7daa00b05dfdb02e2be06f0603f0bde46a0b9efc8f028880b6682c0e47d94735`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "host_resources",
  "description": "Read offline host RAM, available parallelism and current-process Linux cgroup memory bounds through an owned adapter over pinned MIT sysinfo. Unknown process limits fail capacity admission on Linux; other platforms disclose unobserved process limits. Reports estimates, not guaranteed model fit. GPU memory is unknown; do not add host RAM to GPU RAM. No process list, command lines, environment, hostname, credentials, network, downloads, model selection, execution or promotion. Requires the actor's current host_resources grant.",
  "source": "mcp/src/functions/host_resources.rs",
  "phases": [
    "planner",
    "worker"
  ],
  "read_only": true,
  "deterministic": false,
  "external_effects": false,
  "runtime_requirements": {
    "call_timeout_seconds": 10,
    "installed_rustc": false
  },
  "data_input_fields": [],
  "file_input_fields": [],
  "provider_cost_usd": 0,
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "resultSchema": {
    "additionalProperties": false,
    "properties": {
      "architecture": {
        "type": "string"
      },
      "available_parallelism": {
        "oneOf": [
          {
            "minimum": 0,
            "type": "integer"
          },
          {
            "type": "null"
          }
        ]
      },
      "effective_available_bytes": {
        "oneOf": [
          {
            "minimum": 0,
            "type": "integer"
          },
          {
            "type": "null"
          }
        ]
      },
      "effective_limit_bytes": {
        "oneOf": [
          {
            "minimum": 0,
            "type": "integer"
          },
          {
            "type": "null"
          }
        ]
      },
      "gpu_memory_bytes": {
        "type": "null"
      },
      "host_available_bytes": {
        "minimum": 0,
        "type": "integer"
      },
      "host_total_bytes": {
        "minimum": 0,
        "type": "integer"
      },
      "observed_at_ms": {
        "minimum": 0,
        "type": "integer"
      },
      "os": {
        "type": "string"
      },
      "probe": {
        "type": "string"
      },
      "process_available_bytes": {
        "oneOf": [
          {
            "minimum": 0,
            "type": "integer"
          },
          {
            "type": "null"
          }
        ]
      },
      "process_limit_bytes": {
        "oneOf": [
          {
            "minimum": 0,
            "type": "integer"
          },
          {
            "type": "null"
          }
        ]
      },
      "process_limit_status": {
        "enum": [
          "observed",
          "unavailable",
          "not_observed_on_this_platform"
        ],
        "type": "string"
      },
      "provider_cost_usd": {
        "const": 0
      }
    },
    "required": [
      "observed_at_ms",
      "os",
      "architecture",
      "available_parallelism",
      "host_total_bytes",
      "host_available_bytes",
      "process_limit_status",
      "process_limit_bytes",
      "process_available_bytes",
      "effective_limit_bytes",
      "effective_available_bytes",
      "gpu_memory_bytes",
      "probe",
      "provider_cost_usd"
    ],
    "type": "object"
  }
}
```

</details>

## image generate

**Recipe: Create a small image locally.** Freeze an approved confined output_path and grant image_generate to the assigned worker. Supply prompt, dimensions, seed and the bounded step count. The installed pinned FLUX model runs locally with no network or downloads. Inspect the created PNG hash and dimensions; a valid PNG does not establish that the image matches the prompt. Existing destinations and frozen inputs cannot be overwritten.

Implementation: [mcp/src/functions/media_generate.rs](../mcp/src/functions/media_generate.rs). Descriptor snapshot SHA-256: `033f64ebda2f48f7f47d34fe55abf7b9f2223ec215cf647eae765ea09e685439`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "image_generate",
  "description": "Generate one local PNG from a text prompt using the pinned Apache-2.0 FLUX.2 Klein 4-bit model through the owned MFLUX adapter. Fixed four steps, 256..512 pixel dimensions, explicit seed, <=8 MiB. Requires current worker grant and approved active job. Offline isolated process, observed free-memory floor, new confined output path only; no overwrite, downloads, paid fallback, tool grants or automatic semantic acceptance. Current isolation supports macOS only.",
  "source": "mcp/src/functions/media_generate.rs",
  "phases": [
    "worker"
  ],
  "read_only": false,
  "deterministic": false,
  "external_effects": true,
  "runtime_requirements": {
    "call_timeout_seconds": 180,
    "installed_rustc": false
  },
  "data_input_fields": [],
  "file_input_fields": [],
  "provider_cost_usd": 0,
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "height": {
        "enum": [
          256,
          384,
          512
        ],
        "type": "integer"
      },
      "path": {
        "maxLength": 240,
        "minLength": 1,
        "type": "string"
      },
      "prompt": {
        "maxLength": 1200,
        "minLength": 1,
        "type": "string"
      },
      "seed": {
        "maximum": 2147483647,
        "minimum": 0,
        "type": "integer"
      },
      "width": {
        "enum": [
          256,
          384,
          512
        ],
        "type": "integer"
      }
    },
    "required": [
      "prompt",
      "path",
      "width",
      "height",
      "seed"
    ],
    "type": "object"
  },
  "resultSchema": {
    "additionalProperties": false,
    "properties": {
      "artifact": {
        "additionalProperties": false,
        "properties": {
          "bytes": {
            "maximum": 8388608,
            "minimum": 1,
            "type": "integer"
          },
          "created_new": {
            "const": true
          },
          "path": {
            "type": "string"
          },
          "retry_rule": {
            "type": "string"
          },
          "sha256": {
            "pattern": "^[a-f0-9]{64}$",
            "type": "string"
          }
        },
        "required": [
          "path",
          "bytes",
          "sha256",
          "created_new",
          "retry_rule"
        ],
        "type": "object"
      },
      "engine": {
        "const": "mflux_flux2_klein_4bit"
      },
      "height": {
        "enum": [
          256,
          384,
          512
        ],
        "type": "integer"
      },
      "mime_type": {
        "const": "image/png"
      },
      "model_manifest_sha256": {
        "pattern": "^[a-f0-9]{64}$",
        "type": "string"
      },
      "provider_cost_usd": {
        "const": 0
      },
      "request_sha256": {
        "pattern": "^[a-f0-9]{64}$",
        "type": "string"
      },
      "semantic_acceptance": {
        "const": false
      },
      "width": {
        "enum": [
          256,
          384,
          512
        ],
        "type": "integer"
      }
    },
    "required": [
      "artifact",
      "mime_type",
      "engine",
      "request_sha256",
      "model_manifest_sha256",
      "provider_cost_usd",
      "semantic_acceptance",
      "width",
      "height"
    ],
    "type": "object"
  }
}
```

</details>

## integer arithmetic

**Recipe: Calculate exact integer rows.** Supply bounded calculations with stable IDs, integer values and an explicit subtract field. Use the actual returned row/field through approved bindings; do not calculate an expected answer during CEO planning.

Implementation: [mcp/src/functions/arithmetic.rs](../mcp/src/functions/arithmetic.rs). Descriptor snapshot SHA-256: `1f584e19622b47585e07fb40d8d7e16a7f792c8db9821c037d124443cb12dca5`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "integer_arithmetic",
  "description": "Exact bounded integer sums and differences. Returns {calculations:[{id,sum,difference}],exact:true}; difference = sum - subtract. Preserves each id and row order. Output does not echo input values or subtract fields. No code evaluation or policy decision.",
  "source": "mcp/src/functions/arithmetic.rs",
  "phases": [
    "planner",
    "worker"
  ],
  "read_only": true,
  "deterministic": true,
  "external_effects": false,
  "runtime_requirements": {
    "call_timeout_seconds": 10,
    "installed_rustc": false
  },
  "data_input_fields": [],
  "file_input_fields": [],
  "provider_cost_usd": 0,
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "calculations": {
        "items": {
          "additionalProperties": false,
          "properties": {
            "id": {
              "maxLength": 40,
              "minLength": 1,
              "pattern": "^[A-Za-z0-9_-]{1,40}$",
              "type": "string"
            },
            "subtract": {
              "maximum": 10000000,
              "minimum": -10000000,
              "type": "integer"
            },
            "values": {
              "items": {
                "maximum": 10000000,
                "minimum": -10000000,
                "type": "integer"
              },
              "maxItems": 12,
              "type": "array"
            }
          },
          "required": [
            "id",
            "values",
            "subtract"
          ],
          "type": "object"
        },
        "maxItems": 12,
        "minItems": 1,
        "type": "array"
      }
    },
    "required": [
      "calculations"
    ],
    "type": "object"
  },
  "resultSchema": {
    "additionalProperties": false,
    "properties": {
      "calculations": {
        "items": {
          "additionalProperties": false,
          "properties": {
            "difference": {
              "type": "integer"
            },
            "id": {
              "type": "string"
            },
            "sum": {
              "type": "integer"
            }
          },
          "required": [
            "id",
            "sum",
            "difference"
          ],
          "type": "object"
        },
        "maxItems": 12,
        "type": "array"
      },
      "exact": {
        "const": true,
        "type": "boolean"
      }
    },
    "required": [
      "calculations",
      "exact"
    ],
    "type": "object"
  }
}
```

</details>

## memory append

**Recipe: Keep an attributable advisory or experiment.** Supply the bounded body and kind, plus source references where available. The host supplies sender/run/phase/epoch. A record called fact is not automatically verified truth, a permission, or completed-task promotion.

Implementation: [mcp/src/functions/memory.rs](../mcp/src/functions/memory.rs). Descriptor snapshot SHA-256: `8fff77d353f5d74d7c25363b241a8d520cbe6c8a07a43e60a2dbcbad02951239`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "memory_append",
  "description": "Append a bounded private canonical fact/advisory/experiment record. Sender, run, policy epoch and phase come from the host, not tool arguments.",
  "source": "mcp/src/functions/memory.rs",
  "phases": [
    "planner",
    "worker"
  ],
  "read_only": false,
  "deterministic": false,
  "external_effects": false,
  "runtime_requirements": {
    "call_timeout_seconds": 10,
    "installed_rustc": false
  },
  "data_input_fields": [],
  "file_input_fields": [],
  "provider_cost_usd": 0,
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "body": {
        "maxLength": 4096,
        "minLength": 1,
        "type": "string"
      },
      "kind": {
        "enum": [
          "fact",
          "advisory",
          "experiment"
        ],
        "type": "string"
      },
      "source_refs": {
        "items": {
          "additionalProperties": false,
          "properties": {
            "id": {
              "maxLength": 80,
              "type": "string"
            },
            "sha256": {
              "maxLength": 64,
              "minLength": 64,
              "type": "string"
            }
          },
          "required": [
            "id",
            "sha256"
          ],
          "type": "object"
        },
        "maxItems": 8,
        "type": "array"
      }
    },
    "required": [
      "kind",
      "body"
    ],
    "type": "object"
  },
  "resultSchema": null
}
```

</details>

## memory search

**Recipe: Retrieve currently allowed memory.** Supply a literal keyword query and bounded limit. The native policy filters senders/run/epoch before limiting. Check truncated previews and source identities; retrieval cannot expand communication grants.

Implementation: [mcp/src/functions/memory.rs](../mcp/src/functions/memory.rs). Descriptor snapshot SHA-256: `dab5a952fde12817e9526d0b660681ad3e7a73271b0a4e6da3e5aa15f61fd19b`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "memory_search",
  "description": "Retrieve a bounded literal keyword packet from currently authorized senders in this run and policy epoch. Records remain canonical; previews are marked truncated.",
  "source": "mcp/src/functions/memory.rs",
  "phases": [
    "planner",
    "worker"
  ],
  "read_only": true,
  "deterministic": false,
  "external_effects": false,
  "runtime_requirements": {
    "call_timeout_seconds": 10,
    "installed_rustc": false
  },
  "data_input_fields": [],
  "file_input_fields": [],
  "provider_cost_usd": 0,
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "limit": {
        "maximum": 8,
        "minimum": 1,
        "type": "integer"
      },
      "query": {
        "maxLength": 256,
        "type": "string"
      }
    },
    "required": [
      "query"
    ],
    "type": "object"
  },
  "resultSchema": null
}
```

</details>

## owned rust call

**Recipe: Use an admitted pure function.** The owner freezes actor, source/API hashes, function and binding first. Supply binding_id and positional arguments with the documented wire types. Approved dependencies use canonical producer receipts; do not paste invented upstream answers. Inspect unchanged source and independent task QA after native success.

Implementation: [mcp/src/functions/owned_rust_call.rs](../mcp/src/functions/owned_rust_call.rs). Descriptor snapshot SHA-256: `a98a07df51c90bd38eaf9a3d04ce04803073226c4a3f2f142a0d88da88964a6f`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "owned_rust_call",
  "description": "Invoke an owned pure Rust library through a bounded JSON data ABI and installed macOS compiler/runtime sandbox. Provide binding_id and positional arguments only; exact actor, source path/hash, API hash and function must be owner-frozen in required_owned_rust_calls. Literal inputs require input_sha256. Dependent inputs instead require an explicit input_recipe with empty positional placeholders and exact approved producer paths/projections; only canonical current-epoch receipts may fill those slots, with matching wire types and communication grants. Actual input hashes and recipe/receipt lineage are native evidence, never model-supplied authority. Integers are canonical decimal strings; Option uses {some:value}/{none:true}, Result uses {ok:value}/{err:value}. No source authoring, arbitrary paths/flags, shell, dependencies, downloads, network or promotion. Failed invocation is not task completion. Current-epoch successful native evidence and unchanged sources are required by completion; independent task QA remains. A review package ID is owner-declared historical provenance, never native qualification or permission. Unsupported isolation fails closed.",
  "source": "mcp/src/functions/owned_rust_call.rs",
  "phases": [
    "worker"
  ],
  "read_only": true,
  "deterministic": false,
  "external_effects": false,
  "runtime_requirements": {
    "call_timeout_seconds": 110,
    "installed_rustc": true
  },
  "data_input_fields": [
    "arguments"
  ],
  "file_input_fields": [],
  "provider_cost_usd": 0,
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "arguments": {
        "maxItems": 8,
        "type": "array"
      },
      "binding_id": {
        "maxLength": 64,
        "minLength": 1,
        "type": "string"
      }
    },
    "required": [
      "binding_id",
      "arguments"
    ],
    "type": "object"
  },
  "resultSchema": {
    "properties": {
      "output": {},
      "passed": {
        "type": "boolean"
      },
      "promotion_authorized": {
        "const": false
      },
      "provider_cost_usd": {
        "const": 0
      }
    },
    "required": [
      "passed",
      "stage",
      "source_sha256",
      "api_sha256",
      "function",
      "provider_cost_usd",
      "promotion_authorized",
      "binding_id",
      "source_path",
      "input_sha256",
      "owner_declared_review_package_id"
    ],
    "type": "object"
  }
}
```

</details>

## rust verify

**Recipe: Check a quarantined pure Rust candidate.** The owner freezes the separate acceptance executable and hash for the assigned worker. Supply only the confined source_path. Review actual compiler/test diagnostics, current source hash and full readback. Failure remains failure; the tool does not promote source or certify the task.

Implementation: [mcp/src/functions/rust_verify.rs](../mcp/src/functions/rust_verify.rs). Descriptor snapshot SHA-256: `38784e0448639b0174a5470beb50d3a040f754630be9b519456e1594ecbb0bbb`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "rust_verify",
  "description": "Compile and test a bounded pure Rust function library with the installed toolchain and OS isolation. Provide source_path only; actor, acceptance path and acceptance SHA-256 must already be frozen in required_rust_checks by the owner. Candidate syntax: function items only, safe std/core pure modules, no imports/attributes/macros/unsafe/extern/async/I/O/process/environment. No shell, Cargo, dependencies, downloads, network or source promotion. Separate candidate library and owner-frozen acceptance executable; bounded processes/output/time and confined scratch files. Returns passed, source/test hashes and real compiler/test diagnostics. Failed acceptance may include a bounded excerpt of owner-frozen test input only with current files:read permission; the excerpt is untrusted data. Completion requires both a PASS and a full source readback for current bytes and policy epoch; a completed tool receipt alone is insufficient. Currently requires installed macOS sandbox-exec and Rust; unsupported isolation fails closed.",
  "source": "mcp/src/functions/rust_verify.rs",
  "phases": [
    "worker"
  ],
  "read_only": false,
  "deterministic": false,
  "external_effects": true,
  "runtime_requirements": {
    "call_timeout_seconds": 110,
    "installed_rustc": true
  },
  "data_input_fields": [],
  "file_input_fields": [],
  "provider_cost_usd": 0,
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "source_path": {
        "maxLength": 120,
        "minLength": 1,
        "type": "string"
      }
    },
    "required": [
      "source_path"
    ],
    "type": "object"
  },
  "resultSchema": {
    "additionalProperties": false,
    "properties": {
      "compiler": {
        "type": "string"
      },
      "diagnostics": {
        "oneOf": [
          {
            "type": "string"
          },
          {
            "additionalProperties": false,
            "properties": {
              "exit_code": {
                "oneOf": [
                  {
                    "type": "integer"
                  },
                  {
                    "type": "null"
                  }
                ]
              },
              "limit": {
                "oneOf": [
                  {
                    "type": "string"
                  },
                  {
                    "type": "null"
                  }
                ]
              },
              "passed": {
                "type": "boolean"
              },
              "signal": {
                "oneOf": [
                  {
                    "type": "integer"
                  },
                  {
                    "type": "null"
                  }
                ]
              },
              "stderr": {
                "type": "string"
              },
              "stdout": {
                "type": "string"
              }
            },
            "required": [
              "passed",
              "exit_code",
              "signal",
              "limit",
              "stdout",
              "stderr"
            ],
            "type": "object"
          }
        ]
      },
      "failure_context": {
        "additionalProperties": false,
        "properties": {
          "excerpt": {
            "maxLength": 300,
            "type": "string"
          },
          "excerpt_start_char": {
            "minimum": 0,
            "type": "integer"
          },
          "line": {
            "minimum": 1,
            "type": "integer"
          },
          "reported_column": {
            "minimum": 1,
            "type": "integer"
          },
          "scope": {
            "type": "string"
          },
          "tests_path": {
            "type": "string"
          }
        },
        "required": [
          "tests_path",
          "line",
          "reported_column",
          "excerpt_start_char",
          "excerpt",
          "scope"
        ],
        "type": "object"
      },
      "isolation": {
        "type": "string"
      },
      "limits": {
        "type": "object"
      },
      "passed": {
        "type": "boolean"
      },
      "promotion_authorized": {
        "const": false
      },
      "provider_cost_usd": {
        "const": 0
      },
      "source_path": {
        "type": "string"
      },
      "source_sha256": {
        "maxLength": 64,
        "minLength": 64,
        "type": "string"
      },
      "stage": {
        "enum": [
          "admission",
          "compile_candidate",
          "compile_acceptance",
          "acceptance"
        ],
        "type": "string"
      },
      "tests_count": {
        "maximum": 16,
        "minimum": 1,
        "type": "integer"
      },
      "tests_sha256": {
        "maxLength": 64,
        "minLength": 64,
        "type": "string"
      }
    },
    "required": [
      "passed",
      "stage",
      "diagnostics",
      "source_path",
      "source_sha256",
      "tests_sha256",
      "provider_cost_usd",
      "promotion_authorized"
    ],
    "type": "object"
  }
}
```

</details>

## skill list

**Recipe: Discover assigned instructions.** Use an empty object. The result lists only reviewed skills assigned to this actor and its process-bound planner/worker phase. The implementation catalog is not a substitute for these assignments.

Implementation: [mcp/src/functions/skills.rs](../mcp/src/functions/skills.rs). Descriptor snapshot SHA-256: `e9d95c1f20279163e5fc80619f2050f6f4fa87926d081427cb652704cbd3e155`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "skill_list",
  "description": "List reviewed skills assigned to this runtime actor and planner/worker phase; no execution or new authority.",
  "source": "mcp/src/functions/skills.rs",
  "phases": [
    "planner",
    "worker"
  ],
  "read_only": true,
  "deterministic": false,
  "external_effects": false,
  "runtime_requirements": {
    "call_timeout_seconds": 10,
    "installed_rustc": false
  },
  "data_input_fields": [],
  "file_input_fields": [],
  "provider_cost_usd": 0,
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "resultSchema": null
}
```

</details>

## skill read

**Recipe: Read one assigned instruction set.** Use an ID returned by the authorized skill list. Instructions can guide work but never install policy, authorize a tool, broaden data scope or override the frozen task.

Implementation: [mcp/src/functions/skills.rs](../mcp/src/functions/skills.rs). Descriptor snapshot SHA-256: `8b5e5b149f0656b45c42b8e724ed91de24cf21402211d3f07b13a5e114faa7d1`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "skill_read",
  "description": "Read an assigned reviewed skill valid for this process-bound phase; instructions never expand tool, data or budget permissions.",
  "source": "mcp/src/functions/skills.rs",
  "phases": [
    "planner",
    "worker"
  ],
  "read_only": true,
  "deterministic": false,
  "external_effects": false,
  "runtime_requirements": {
    "call_timeout_seconds": 10,
    "installed_rustc": false
  },
  "data_input_fields": [],
  "file_input_fields": [],
  "provider_cost_usd": 0,
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "id": {
        "maxLength": 100,
        "minLength": 1,
        "type": "string"
      }
    },
    "required": [
      "id"
    ],
    "type": "object"
  },
  "resultSchema": null
}
```

</details>

## table aggregate

**Recipe: Group exact integer data.** Supply flat scalar rows, group_by, value_field and an explicit filter or null. Preserve duplicate rows and zero-total groups. Feed actual /groups or /total through approved source bindings; unsupported floats/coercion/expressions fail rather than being guessed.

Implementation: [mcp/src/functions/table.rs](../mcp/src/functions/table.rs). Descriptor snapshot SHA-256: `79e1a004c1f5f7a2e3fadd54d3dd3c42c946c5bc26dab65a053484c302e65da2`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "table_aggregate",
  "description": "Pure bounded flat-table aggregation: exact typed equality filter (null means no filter), group_by and integer value_field. Returns input_rows,matched_rows,excluded_rows,groups:[{key,rows,total}],total,exact:true. Group keys use canonical JSON lexical order; duplicates count as separate rows and zero-total groups remain. Every selected field must exist on every input row, even excluded rows. No floats, coercion, expressions, filesystem or network. Use /groups with json encoding or /total with integer_text for artifacts; never invent aggregate answers during planning.",
  "source": "mcp/src/functions/table.rs",
  "phases": [
    "planner",
    "worker"
  ],
  "read_only": true,
  "deterministic": true,
  "external_effects": false,
  "runtime_requirements": {
    "call_timeout_seconds": 10,
    "installed_rustc": false
  },
  "data_input_fields": [
    "rows"
  ],
  "file_input_fields": [],
  "provider_cost_usd": 0,
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "filter": {
        "oneOf": [
          {
            "type": "null"
          },
          {
            "additionalProperties": false,
            "properties": {
              "equals": {
                "oneOf": [
                  {
                    "maxLength": 128,
                    "type": "string"
                  },
                  {
                    "maximum": 1000000000000,
                    "minimum": -1000000000000,
                    "type": "integer"
                  },
                  {
                    "type": "boolean"
                  },
                  {
                    "type": "null"
                  }
                ]
              },
              "field": {
                "maxLength": 40,
                "minLength": 1,
                "type": "string"
              }
            },
            "required": [
              "field",
              "equals"
            ],
            "type": "object"
          }
        ]
      },
      "group_by": {
        "maxLength": 40,
        "minLength": 1,
        "type": "string"
      },
      "rows": {
        "items": {
          "additionalProperties": {
            "oneOf": [
              {
                "maxLength": 128,
                "type": "string"
              },
              {
                "maximum": 1000000000000,
                "minimum": -1000000000000,
                "type": "integer"
              },
              {
                "type": "boolean"
              },
              {
                "type": "null"
              }
            ]
          },
          "maxProperties": 16,
          "minProperties": 1,
          "type": "object"
        },
        "maxItems": 64,
        "type": "array"
      },
      "value_field": {
        "maxLength": 40,
        "minLength": 1,
        "type": "string"
      }
    },
    "required": [
      "rows",
      "filter",
      "group_by",
      "value_field"
    ],
    "type": "object"
  },
  "resultSchema": {
    "additionalProperties": false,
    "properties": {
      "exact": {
        "const": true,
        "type": "boolean"
      },
      "excluded_rows": {
        "type": "integer"
      },
      "groups": {
        "items": {
          "additionalProperties": false,
          "properties": {
            "key": {
              "oneOf": [
                {
                  "type": "string"
                },
                {
                  "type": "integer"
                },
                {
                  "type": "boolean"
                },
                {
                  "type": "null"
                }
              ]
            },
            "rows": {
              "type": "integer"
            },
            "total": {
              "type": "integer"
            }
          },
          "required": [
            "key",
            "rows",
            "total"
          ],
          "type": "object"
        },
        "maxItems": 64,
        "type": "array"
      },
      "input_rows": {
        "type": "integer"
      },
      "matched_rows": {
        "type": "integer"
      },
      "total": {
        "type": "integer"
      }
    },
    "required": [
      "input_rows",
      "matched_rows",
      "excluded_rows",
      "groups",
      "total",
      "exact"
    ],
    "type": "object"
  }
}
```

</details>

## table join

**Recipe: Combine two bounded flat tables.** Choose inner or left based on the confirmed treatment of unmatched records. Supply exact typed keys and explicit output columns. Preserve duplicate matches and native output ordering. Use actual /rows downstream; missing fields and output overflow must stop the task.

Implementation: [mcp/src/functions/table_join.rs](../mcp/src/functions/table_join.rs). Descriptor snapshot SHA-256: `a7a0d64318471403420659af57d551be16d487cdb0ce443ddc2be44fbb4f336b`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "table_join",
  "description": "Pure bounded inner/left join of left_rows and right_rows (each <=64 flat scalar records). left_key/right_key use exact JSON scalar equality: booleans, strings and integers stay distinct; null matches null. Duplicate matches multiply; never deduplicate. columns:[{side:left|right,field,as}] explicitly selects and renames 1..16 unique output fields. Every key/selected field must exist on every corresponding input row, including unmatched rows. Inner joins discard unmatched left records, but unmatched_left_rows still counts those original left records. Left joins preserve unmatched left records and project absent selected right fields as null. Choose inner when unmatched input records must be discarded, and left when they must be retained. Stable order: original left rows, then original matching right rows. Returns rows,left_rows,right_rows,matched_left_rows,unmatched_left_rows,exact:true. At most 64 output rows and 16384 input/output bytes; overflow fails without a successful partial result. No coercion, expressions, I/O, network or policy decisions. Feed actual /rows into downstream tools via approved argument_sources.",
  "source": "mcp/src/functions/table_join.rs",
  "phases": [
    "planner",
    "worker"
  ],
  "read_only": true,
  "deterministic": true,
  "external_effects": false,
  "runtime_requirements": {
    "call_timeout_seconds": 10,
    "installed_rustc": false
  },
  "data_input_fields": [
    "left_rows",
    "right_rows"
  ],
  "file_input_fields": [],
  "provider_cost_usd": 0,
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "columns": {
        "items": {
          "additionalProperties": false,
          "properties": {
            "as": {
              "maxLength": 40,
              "minLength": 1,
              "type": "string"
            },
            "field": {
              "maxLength": 40,
              "minLength": 1,
              "type": "string"
            },
            "side": {
              "enum": [
                "left",
                "right"
              ],
              "type": "string"
            }
          },
          "required": [
            "side",
            "field",
            "as"
          ],
          "type": "object"
        },
        "maxItems": 16,
        "minItems": 1,
        "type": "array"
      },
      "kind": {
        "enum": [
          "inner",
          "left"
        ],
        "type": "string"
      },
      "left_key": {
        "maxLength": 40,
        "minLength": 1,
        "type": "string"
      },
      "left_rows": {
        "items": {
          "additionalProperties": {
            "oneOf": [
              {
                "maxLength": 128,
                "type": "string"
              },
              {
                "maximum": 1000000000000,
                "minimum": -1000000000000,
                "type": "integer"
              },
              {
                "type": "boolean"
              },
              {
                "type": "null"
              }
            ]
          },
          "maxProperties": 16,
          "minProperties": 1,
          "type": "object"
        },
        "maxItems": 64,
        "type": "array"
      },
      "right_key": {
        "maxLength": 40,
        "minLength": 1,
        "type": "string"
      },
      "right_rows": {
        "items": {
          "additionalProperties": {
            "oneOf": [
              {
                "maxLength": 128,
                "type": "string"
              },
              {
                "maximum": 1000000000000,
                "minimum": -1000000000000,
                "type": "integer"
              },
              {
                "type": "boolean"
              },
              {
                "type": "null"
              }
            ]
          },
          "maxProperties": 16,
          "minProperties": 1,
          "type": "object"
        },
        "maxItems": 64,
        "type": "array"
      }
    },
    "required": [
      "left_rows",
      "right_rows",
      "left_key",
      "right_key",
      "kind",
      "columns"
    ],
    "type": "object"
  },
  "resultSchema": {
    "additionalProperties": false,
    "properties": {
      "exact": {
        "const": true,
        "type": "boolean"
      },
      "left_rows": {
        "type": "integer"
      },
      "matched_left_rows": {
        "type": "integer"
      },
      "right_rows": {
        "type": "integer"
      },
      "rows": {
        "items": {
          "additionalProperties": {
            "oneOf": [
              {
                "maxLength": 128,
                "type": "string"
              },
              {
                "maximum": 1000000000000,
                "minimum": -1000000000000,
                "type": "integer"
              },
              {
                "type": "boolean"
              },
              {
                "type": "null"
              }
            ]
          },
          "maxProperties": 16,
          "minProperties": 1,
          "type": "object"
        },
        "maxItems": 64,
        "type": "array"
      },
      "unmatched_left_rows": {
        "type": "integer"
      }
    },
    "required": [
      "rows",
      "left_rows",
      "right_rows",
      "matched_left_rows",
      "unmatched_left_rows",
      "exact"
    ],
    "type": "object"
  }
}
```

</details>

## table read json

**Recipe: Import a complete bounded JSON table.** Freeze the confined input path and, when required, complete source hash. Check returned source identity/size and preserve row order, duplicates and scalar types. A truncated page, duplicate decoded key or unsupported nested/float value imports nothing.

Implementation: [mcp/src/functions/table_read.rs](../mcp/src/functions/table_read.rs). Descriptor snapshot SHA-256: `9ab0df32e8754a5934b87b728bc92595946606e2197025a64e0a64c943497b1d`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "table_read_json",
  "description": "Read one complete UTF-8 JSON array of flat scalar records from a confined run-workspace path, at most 8192 source bytes, 64 rows, 16 fields/row. Rejects truncated pages, trailing data, duplicate decoded keys, floats, nested values and invalid field names. Returns rows (for a table_aggregate rows argument_source), source_path, source_sha256 and source_bytes. Preserves row order, duplicates and scalar types; never computes totals or executes content. Optional expected_sha256 pins complete source bytes to an approved lowercase SHA-256; a mismatch imports nothing. Read-only specific authority; no arbitrary filesystem paths, symlinks, subprocesses or network.",
  "source": "mcp/src/functions/table_read.rs",
  "phases": [
    "planner",
    "worker"
  ],
  "read_only": true,
  "deterministic": false,
  "external_effects": false,
  "runtime_requirements": {
    "call_timeout_seconds": 10,
    "installed_rustc": false
  },
  "data_input_fields": [],
  "file_input_fields": [
    "path"
  ],
  "provider_cost_usd": 0,
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "expected_sha256": {
        "maxLength": 64,
        "minLength": 64,
        "type": "string"
      },
      "path": {
        "maxLength": 2000,
        "minLength": 1,
        "type": "string"
      }
    },
    "required": [
      "path"
    ],
    "type": "object"
  },
  "resultSchema": {
    "additionalProperties": false,
    "properties": {
      "rows": {
        "items": {
          "additionalProperties": {
            "oneOf": [
              {
                "maxLength": 128,
                "type": "string"
              },
              {
                "maximum": 1000000000000,
                "minimum": -1000000000000,
                "type": "integer"
              },
              {
                "type": "boolean"
              },
              {
                "type": "null"
              }
            ]
          },
          "maxProperties": 16,
          "minProperties": 1,
          "type": "object"
        },
        "maxItems": 64,
        "type": "array"
      },
      "source_bytes": {
        "maximum": 8192,
        "minimum": 0,
        "type": "integer"
      },
      "source_path": {
        "type": "string"
      },
      "source_sha256": {
        "maxLength": 64,
        "minLength": 64,
        "type": "string"
      }
    },
    "required": [
      "rows",
      "source_path",
      "source_sha256",
      "source_bytes"
    ],
    "type": "object"
  }
}
```

</details>

## What is not included

`finish` is a host submission controller, not a native tool in this catalog. It cannot self-approve a task. Local PNG generation and generic text-to-speech are explicit tools above. PDF extraction, image conversion, audio transcription and video creation are not implied by these tools or model filenames; those expansions remain unimplemented. Media generation currently needs the installed macOS isolation adapter, pinned local models and dependencies; see [local media setup](../mcp/media/README.md). The output judge infers requested media from the prompt without a media-mode switch, but cannot grant tools or hide a missing engine. Model-download/sprite-generation tools and the governed builder/progress API remain follow-on work.

Generated-code verification/invocation currently requires the installed macOS isolation adapter; unsupported platforms fail closed. Native confined file support and resource observations have their own platform limits. Models and open-source components retain their own licenses/provenance obligations.

The exact schemas below are implementation declarations, not complete semantic preconditions. Frozen task contracts, current native policy, source confinement and independent acceptance remain authoritative. Refresh this document from a reviewed current catalog when implementations change; the generator refuses an unreviewed tool-name set. The current catalog snapshot is not a release attestation.
