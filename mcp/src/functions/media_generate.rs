//! Thin governed tool adapters. No mode selector and no prompt-created authority.
use crate::{
    contracts::{EmbeddedContracts, ToolContracts},
    media, workspace, Phase, RuntimeRequirements, Tool, ToolContext,
};
use anyhow::{Context, Result};
use serde_json::{json, Value};
pub struct ImageGenerate;
pub struct AudioGenerate;
fn generate(context: &ToolContext<'_>, mut args: Value, kind: &str) -> Result<Value> {
    let field = if kind == "image" { "prompt" } else { "text" };
    crate::safety::require_persistable(args[field].as_str().context("media prompt missing")?)?;
    args["kind"] = json!(kind);
    let root = context.authority.workspace(context.scope)?;
    media::publish(
        workspace::open(&root, true)?.as_ref(),
        &args,
        &media::LocalBackend,
    )
}
macro_rules! tool {
    ($struct:ty,$name:literal,$kind:literal,$description:literal) => {
        impl Tool for $struct {
            fn name(&self) -> &'static str {
                $name
            }
            fn source(&self) -> &'static str {
                file!()
            }
            fn description(&self) -> &'static str {
                $description
            }
            fn schema(&self) -> Value {
                EmbeddedContracts.input_schema(self.name()).unwrap()
            }
            fn result_schema(&self, _: Option<&Value>) -> Option<Value> {
                Some(media::result_schema($kind))
            }
            fn read_only(&self) -> bool {
                false
            }
            fn external_effects(&self) -> bool {
                true
            }
            fn file_output_fields(&self) -> &'static [&'static str] {
                &["path"]
            }
            fn phases(&self) -> &'static [Phase] {
                &[Phase::Worker]
            }
            fn runtime_requirements(&self) -> RuntimeRequirements {
                RuntimeRequirements {
                    installed_rustc: false,
                    installed_python: false,
                    call_timeout_seconds: 180,
                }
            }
            fn execute(&self, context: &ToolContext<'_>, args: Value) -> Result<Value> {
                EmbeddedContracts.validate(self.name(), &args)?;
                generate(context, args, $kind)
            }
        }
    };
}
tool!(ImageGenerate,"image_generate","image","Generate one local PNG from a text prompt using the pinned Apache-2.0 FLUX.2 Klein 4-bit model through the owned MFLUX adapter. Fixed four steps, 256..512 pixel dimensions, explicit seed, <=8 MiB. Requires current worker grant and approved active job. Offline isolated process, observed free-memory floor, new confined output path only; no overwrite, downloads, paid fallback, tool grants or automatic semantic acceptance. Current isolation supports macOS only.");
tool!(AudioGenerate,"audio_generate","audio","Generate one local mono PCM16 WAV narration from <=600 characters using the pinned Piper/LJ Speech CPU adapter. Fixed generic English voice; no cloning, music, transcription or identity claim. Requires current worker grant and approved active job. Offline isolated process, <=8 MiB, new confined output path only; no overwrite, downloads, paid fallback or automatic semantic acceptance. Current isolation supports macOS only.");
