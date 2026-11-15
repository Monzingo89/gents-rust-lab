//! Thin governed workspace adapter; WAV parsing/statistics live in audio.rs.
use crate::{
    audio,
    contracts::{EmbeddedContracts, ToolContracts},
    workspace, Tool, ToolContext,
};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
pub struct AudioInspect;
impl Tool for AudioInspect {
    fn name(&self) -> &'static str {
        "audio_inspect"
    }
    fn source(&self) -> &'static str {
        file!()
    }
    fn description(&self) -> &'static str {
        "Inspect one complete hash-pinned confined .wav file (<=16 MiB), little-endian mono/stereo integer PCM16 only. Returns source provenance, exact frame/rate duration, per-channel extrema, peak absolute sample, sum, zero/rail counts and adjacent nonzero sign changes. Rejects malformed/ambiguous headers, partial frames and unsupported codecs. Ignores unknown metadata payloads without echoing them. Read-only current actor grant required. No playback, transcription, speaker/content/health inference, network, executable codecs, downloads or source modification. Rail hits alone do not prove clipping."
    }
    fn schema(&self) -> Value {
        EmbeddedContracts.input_schema(self.name()).unwrap()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn file_input_fields(&self) -> &'static [&'static str] {
        &["path"]
    }
    fn result_schema(&self, _: Option<&Value>) -> Option<Value> {
        let count = json!({"type":"integer","minimum":0,"maximum":8388608});
        let sample = json!({"type":"integer","minimum":-32768,"maximum":32767});
        Some(
            json!({"type":"object","additionalProperties":false,"required":["source_path","source_sha256","source_bytes","format","channels","sample_rate_hz","bits_per_sample","frame_count","duration_seconds","channel_statistics","all_samples_zero","measurement_scope","provider_cost_usd"],"properties":{
                "source_path":{"type":"string"},"source_sha256":{"type":"string","minLength":64,"maxLength":64},"source_bytes":{"type":"integer","minimum":0,"maximum":audio::MAX_BYTES},"provider_cost_usd":{"const":0},
                "format":{"const":"wav_pcm16_le"},"channels":{"type":"integer","minimum":1,"maximum":2},"bits_per_sample":{"const":16},"sample_rate_hz":{"type":"integer","minimum":1,"maximum":4294967295u64},"frame_count":count,
                "duration_seconds":{"type":"object","additionalProperties":false,"required":["numerator","denominator"],"properties":{"numerator":count,"denominator":{"type":"integer","minimum":1,"maximum":4294967295u64}}},
                "all_samples_zero":{"type":"boolean"},"measurement_scope":{"type":"string"},
                "channel_statistics":{"type":"array","minItems":1,"maxItems":2,"items":{"type":"object","additionalProperties":false,"required":["channel","minimum","maximum","peak_abs","sample_sum","zero_samples","rail_samples","adjacent_sign_changes"],"properties":{
                    "channel":{"type":"integer","minimum":0,"maximum":1},"minimum":{"oneOf":[sample,{"type":"null"}]},"maximum":{"oneOf":[sample,{"type":"null"}]},"peak_abs":{"type":"integer","minimum":0,"maximum":32768},"sample_sum":{"type":"integer","minimum":-274877906944i64,"maximum":274877906944i64},"zero_samples":count,"rail_samples":count,"adjacent_sign_changes":count
                }}}
            }}),
        )
    }
    fn execute(&self, context: &ToolContext<'_>, args: Value) -> Result<Value> {
        EmbeddedContracts.validate(self.name(), &args)?;
        let path = args["path"].as_str().context("audio source path missing")?;
        let expected = args["expected_sha256"]
            .as_str()
            .context("audio source digest missing")?;
        ensure!(
            path.to_ascii_lowercase().ends_with(".wav"),
            "audio_inspect requires a WAV workspace path"
        );
        ensure!(
            expected.len() == 64
                && expected
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "expected_sha256 must be lowercase SHA-256"
        );
        let root = context.authority.workspace(context.scope)?;
        let bytes = workspace::open(&root, false)?.read_bytes_complete(path, audio::MAX_BYTES)?;
        let digest = crate::digest(&bytes);
        ensure!(
            digest == expected,
            "audio source digest differs from approved bytes; no measurements imported"
        );
        let mut result = audio::inspect_pcm16(&bytes)?;
        result["source_path"] = json!(path);
        result["source_sha256"] = json!(digest);
        result["source_bytes"] = json!(bytes.len());
        result["provider_cost_usd"] = json!(0);
        Ok(result)
    }
}
