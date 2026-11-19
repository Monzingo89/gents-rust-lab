# Owned native MCP cookbook

This covers all 16 tools in the local native implementation catalog captured on 2026-11-15. It is a source-derived recipe/reference, not a list of an agent's grants, proof that every task works, a security attestation or a migration claim for legacy Python tools. Tool calls target $0 provider fees; models, hardware and electricity are separate.

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
| [files](#files) | Read, write, list or rename a task file | planner, worker |
| [host_resources](#host-resources) | Check local capacity before choosing a model | planner, worker |
| [image_generate](#image-generate) | Create a small image locally | worker |
| [integer_arithmetic](#integer-arithmetic) | Calculate exact integer rows | planner, worker |
| [memory_append](#memory-append) | Keep an attributable advisory or experiment | planner, worker |
| [memory_search](#memory-search) | Retrieve currently allowed memory | planner, worker |
| [owned_rust_call](#owned-rust-call) | Use an admitted pure function | worker |
| [python_check](#python-check) | Retest copied Python source after a repair | worker |
| [rust_verify](#rust-verify) | Check a quarantined pure Rust candidate | worker |
| [skill_list](#skill-list) | Discover assigned instructions | planner, worker |
| [skill_read](#skill-read) | Read one assigned instruction set | planner, worker |
| [table_aggregate](#table-aggregate) | Group exact integer data | planner, worker |
| [table_join](#table-join) | Combine two bounded flat tables | planner, worker |
| [table_read_json](#table-read-json) | Import a complete bounded JSON table | planner, worker |

## audio generate

**Recipe: Speak a short script locally.** Freeze an approved confined output_path and grant audio_generate to the assigned worker. Supply the exact text and supported local voice. The installed pinned Piper model creates a WAV without network or downloads. Inspect the actual receipt and WAV properties; the generic voice is not cloned, and speech quality still needs listening. Existing destinations and frozen inputs cannot be overwritten.

Implementation: [mcp/src/functions/media_generate.rs](../mcp/src/functions/media_generate.rs). Descriptor snapshot SHA-256: `5cff6f43a102b751a83fef0e13af4c2500505332f416a00451784f06576cfbb1`.

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
    "installed_python": false,
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

Implementation: [mcp/src/functions/audio_inspect.rs](../mcp/src/functions/audio_inspect.rs). Descriptor snapshot SHA-256: `9b9cd4acdae44a2f6db23e7fce46e5aca3c267e7f6bd7af0b5996ec15d717503`.

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
    "installed_python": false,
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

**Recipe: Read, write, list or rename a task file.** Use explicit operation grants. Read bounded UTF-8 pages, or use read metadata_only=true without pagination to fingerprint bounded text/binary without exposing its body. Worker-only rename requires files:rename plus files:read, the exact source expected_sha256, an absent destination and existing parents. It preserves regular-file bytes/mode up to 16 MiB; no overwrite, links, directory moves or copy/delete fallback. Frozen inputs cannot be moved or replaced. Lost/error receipts require BOTH paths/hash and canonical audit reconciliation; the harness retains unconfirmed rename intents and stops.

Implementation: [mcp/src/functions/files.rs](../mcp/src/functions/files.rs). Descriptor snapshot SHA-256: `91a5c525450f0c158023ba67d8104bad5d9df6fb0b851decbe685b5d7dfd6567`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "files",
  "description": "Governed run-workspace files: read UTF-8 pages, atomically replace bounded text, list safe entries, or exclusively rename a regular text/binary file up to 16 MiB. Read metadata_only=true omits body/decoding and returns the complete file hash and byte count; omit pagination. Rename requires files:rename plus read authority, an exact source expected_sha256, an absent destination and existing parents; it preserves bytes/mode and never overwrites or emulates unsupported atomic flags. Optional write expected_sha256 also requires read authority. MCP effects are serialized; frozen owner inputs cannot be overwritten or moved. No symlinks, hard links, subprocesses or arbitrary paths. Planner phase cannot mutate. Lost/error receipts require BOTH rename paths and hash inspection before retry.",
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
    "installed_python": false,
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
          "metadata_only": {
            "type": "boolean"
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
      },
      {
        "additionalProperties": false,
        "properties": {
          "destination": {
            "maxLength": 2000,
            "minLength": 1,
            "type": "string"
          },
          "expected_sha256": {
            "maxLength": 64,
            "minLength": 64,
            "pattern": "^[0-9a-f]{64}$",
            "type": "string"
          },
          "op": {
            "const": "rename"
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
          "destination",
          "expected_sha256"
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

Implementation: [mcp/src/functions/host_resources.rs](../mcp/src/functions/host_resources.rs). Descriptor snapshot SHA-256: `a308486eb64eace3ba95badc258d40c4870ac2dd526f7a592974ba3269be8aa5`.

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
    "installed_python": false,
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

Implementation: [mcp/src/functions/media_generate.rs](../mcp/src/functions/media_generate.rs). Descriptor snapshot SHA-256: `04580b2c3cc3f20f200fc0f6c50cba1fd17f497dc163f2f2b37544de5dd1b7d2`.

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
    "installed_python": false,
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

Implementation: [mcp/src/functions/arithmetic.rs](../mcp/src/functions/arithmetic.rs). Descriptor snapshot SHA-256: `fcf9278d7b9e2c8434e17add0b64a419e2ffbf57cc64bf5a8f512e3de070de76`.

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
    "installed_python": false,
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

Implementation: [mcp/src/functions/memory.rs](../mcp/src/functions/memory.rs). Descriptor snapshot SHA-256: `2f562f7fc09f67f9c707b164b75323f33a4c494b77a2d8e73b29ee7f188dff32`.

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
    "installed_python": false,
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

Implementation: [mcp/src/functions/memory.rs](../mcp/src/functions/memory.rs). Descriptor snapshot SHA-256: `5a41815772dd6cd334537096076d5d4b69eb1eddb522062aac13ef901a786596`.

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
    "installed_python": false,
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

Implementation: [mcp/src/functions/owned_rust_call.rs](../mcp/src/functions/owned_rust_call.rs). Descriptor snapshot SHA-256: `7964fd07ef9b0b387ba6031d7e99dacb1ecd9a66ec6967d4917cf24b6a1f16d3`.

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
    "installed_python": false,
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

## python check

**Recipe: Retest copied Python source after a repair.** The owner freezes explicit source paths, a top-level unittest suite/hash, actor and installed interpreter hash in python_check_contracts. Supply contract_id; the host binds the current source identity before independent review. Each invocation executes fresh copied sources. frozen_inputs pins immutable dependencies; scratch_bytes defaults to zero and may admit bounded isolated temporary I/O. No shell, network, run-workspace writes, package installation or downloads. Inspect reported_fail diagnostics and repair actual source before checking again. Python can spoof a zero exit: reported_pass is not independent acceptance, source promotion or company completion. Currently macOS only with owner-configured GENTS_PYTHON.

Implementation: [mcp/src/functions/python_check.rs](../mcp/src/functions/python_check.rs). Descriptor snapshot SHA-256: `f1d7f72b5d8e4cb7ebb2893de9ab12ef236e2e7718bfa1a7f80a136eb5b5cbc0`.

<details>
<summary>Exact parameters, result shape, limits and implementation metadata</summary>

```json
{
  "name": "python_check",
  "description": "Owner-scoped local Python unittest DIAGNOSTICS for explicit copied .py inputs. Provide contract_id only; the host binds expected_source_identity. The owner must freeze python_check_contracts with actor, source_paths, tests_path/tests_sha256 and interpreter_sha256. Requires current python_check and files:read grants and installed macOS isolation. Optional frozen_inputs pins immutable source dependencies. scratch_bytes defaults to zero; owner-frozen values through 8388608 allow private scratch data/create/rename/delete I/O with sampled byte/entry limits and no mode changes. No shell, network, run-workspace writes, subprocesses, downloads, result cache, promotion or acceptance. Python can spoof its test exit; reported_pass is not independent correctness or company completion. Source changes permit a new reviewed diagnostic; unchanged state remains retry-limited.",
  "source": "mcp/src/functions/python_check.rs",
  "phases": [
    "worker"
  ],
  "read_only": false,
  "deterministic": false,
  "external_effects": true,
  "runtime_requirements": {
    "call_timeout_seconds": 12,
    "installed_python": true,
    "installed_rustc": false
  },
  "data_input_fields": [],
  "file_input_fields": [],
  "provider_cost_usd": 0,
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "contract_id": {
        "maxLength": 80,
        "minLength": 1,
        "type": "string"
      },
      "expected_source_identity": {
        "maxLength": 64,
        "minLength": 64,
        "type": "string"
      }
    },
    "required": [
      "contract_id"
    ],
    "type": "object"
  },
  "resultSchema": {
    "additionalProperties": false,
    "properties": {
      "authority_granted": {
        "const": false
      },
      "contract_id": {
        "maxLength": 80,
        "minLength": 1,
        "type": "string"
      },
      "elapsed_ms": {
        "minimum": 0,
        "type": "integer"
      },
      "exit_code": {
        "type": "integer"
      },
      "inputs_sha256": {
        "maxProperties": 32,
        "type": "object"
      },
      "interpreter_sha256": {
        "maxLength": 64,
        "minLength": 64,
        "type": "string"
      },
      "model_calls": {
        "const": 0
      },
      "output_truncated": {
        "type": "boolean"
      },
      "promoted": {
        "const": false
      },
      "provider_cost_usd": {
        "const": 0
      },
      "python_version": {
        "type": "string"
      },
      "qualified": {
        "const": false
      },
      "resident_memory_limit": {
        "type": "boolean"
      },
      "scope": {
        "type": "string"
      },
      "scratch_limit_bytes": {
        "maximum": 8388608,
        "minimum": 0,
        "type": "integer"
      },
      "scratch_limit_exceeded": {
        "type": "boolean"
      },
      "scratch_observed_bytes": {
        "minimum": 0,
        "type": "integer"
      },
      "scratch_observed_entries": {
        "minimum": 0,
        "type": "integer"
      },
      "snapshot_unchanged": {
        "type": "boolean"
      },
      "source_identity": {
        "maxLength": 64,
        "minLength": 64,
        "type": "string"
      },
      "status": {
        "enum": [
          "reported_pass",
          "reported_fail",
          "indeterminate"
        ],
        "type": "string"
      },
      "stderr": {
        "maxLength": 16384,
        "type": "string"
      },
      "stdout": {
        "maxLength": 16384,
        "type": "string"
      },
      "suite_sha256": {
        "maxLength": 64,
        "minLength": 64,
        "type": "string"
      },
      "timed_out": {
        "type": "boolean"
      },
      "verification_identity": {
        "maxLength": 64,
        "minLength": 64,
        "type": "string"
      }
    },
    "required": [
      "status",
      "source_identity",
      "inputs_sha256",
      "suite_sha256",
      "snapshot_unchanged",
      "interpreter_sha256",
      "python_version",
      "exit_code",
      "timed_out",
      "resident_memory_limit",
      "output_truncated",
      "stdout",
      "stderr",
      "elapsed_ms",
      "model_calls",
      "provider_cost_usd",
      "qualified",
      "promoted",
      "authority_granted",
      "scope",
      "verification_identity",
      "contract_id",
      "scratch_limit_bytes",
      "scratch_observed_bytes",
      "scratch_observed_entries",
      "scratch_limit_exceeded"
    ],
    "type": "object"
  }
}
```

</details>

## rust verify

**Recipe: Check a quarantined pure Rust candidate.** The owner freezes the separate acceptance executable and hash for the assigned worker. Supply only the confined source_path. Review actual compiler/test diagnostics, current source hash and full readback. Failure remains failure; the tool does not promote source or certify the task.

Implementation: [mcp/src/functions/rust_verify.rs](../mcp/src/functions/rust_verify.rs). Descriptor snapshot SHA-256: `746d60098f8b84ed25312d0d7e62e0e3ed59824abacc77a92252d3d2474f47db`.

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
    "installed_python": false,
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

Implementation: [mcp/src/functions/skills.rs](../mcp/src/functions/skills.rs). Descriptor snapshot SHA-256: `6fa5c17055771f4bb479206b4ed48ea79401d3fd32599b877f8ac94711731121`.

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
    "installed_python": false,
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

Implementation: [mcp/src/functions/skills.rs](../mcp/src/functions/skills.rs). Descriptor snapshot SHA-256: `cd58f89500c5a24352a6ea7af4a5580c79c6e33a426cd8008dc51135a9c6a79b`.

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
    "installed_python": false,
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

Implementation: [mcp/src/functions/table.rs](../mcp/src/functions/table.rs). Descriptor snapshot SHA-256: `87eebf38ce9f339ae22739c4a85269de190081dddd588eff5534eba5db857046`.

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
    "installed_python": false,
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

Implementation: [mcp/src/functions/table_join.rs](../mcp/src/functions/table_join.rs). Descriptor snapshot SHA-256: `e16fa6d10240f0e90ce32b70563458adc1a6c64a5a3bc8847db224835bb3e73c`.

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
    "installed_python": false,
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

Implementation: [mcp/src/functions/table_read.rs](../mcp/src/functions/table_read.rs). Descriptor snapshot SHA-256: `f7254f41b85e5b675b24455f96a581b0ad34ba85a55810481fca613edfba3375`.

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
    "installed_python": false,
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
