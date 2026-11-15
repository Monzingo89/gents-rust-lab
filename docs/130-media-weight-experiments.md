# Media weight experiments

This extends the local weight-improvement goal beyond language. No image, speech,
or video quality improvement has been demonstrated yet.

## Installed training paths checked on November 14, 2026

The installed MFLUX 0.24.0 source has a text-to-image training adapter for the
installed distilled FLUX.2 Klein 4B model and accepts a local model directory.
Its edit-training path separately requires the base variant. The distinction is
also documented in the [upstream FLUX.2 guide](https://github.com/mflux-community/mflux/blob/main/src/mflux/models/flux2/README.md).
We did not download or silently substitute a base model.

The installed Piper asset is an ONNX inference voice. Its [documented training
workflow](https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/TRAINING.md) uses
paired speech/transcripts and training checkpoints, then exports ONNX. A trainable
checkpoint, reviewed paired data and a local speech-quality evaluation pipeline
are still needed. No voice cloning or audio weight modification occurred.
There is still no registered video-generation/training engine.

## Prepared geometric image experiment

`scripts/prepare_image_weight_experiment.py` creates twelve owned procedural PNGs
and captions: four colored squares at three sizes on white backgrounds. These
are calibration references, not an attempt to fabricate user-supplied annotated
photos. Four prospective generation seeds are frozen separately. The evaluation
shares the trained color/shape concepts; it does not test unseen concepts.

`scripts/image_geometry_evaluation.py` checks foreground area, centroid placement,
bounding-box aspect ratio, fill ratio, requested-color coverage and background
whiteness. Thresholds were fixed before model inference. Passing only demonstrates
these geometric properties, not aesthetics, semantic richness or general image
quality. Tests establish that references pass and wrong color, blank, displaced
and non-square fixtures fail.

The prepared run is private under `data/weight-experiments/image-geometry-v1`.
It includes data hashes, source snapshots, configuration and a preflight record.
The fixed configuration requests five epochs over twelve images, rank-four
adaptation of one query projection, batch size one, 256px resolution and low-RAM
checkpointing. No model weights have been loaded or changed by preparation.
Installed MFLUX accepted the configuration in an offline **dry run**; this does
not prove the full training loop fits memory or executes successfully.

All image and speech files in `config/media.local.json` were streamed and compared
with their existing reviewed pins. [Preflight evidence](evidence/media-weight-preflight-2026-11-14.json).
Provider fees were $0. No model download was needed.

## Native baseline result

The opt-in `frozen_image_weight_baseline` MCP integration probe generated all four
images from the frozen manifest. It used fixture owner approval and tool grants,
the normal native image adapter, pinned model verification, the default-deny
macOS sandbox with no network permission, and the cross-process media lock.
The idle local office was stopped for exclusive GPU use and restored afterward;
all seven existing company runs remained terminal. This was an approved test
fixture, not a completed user task.

All **4/4** images passed every frozen geometry criterion. The native probe took
31.59 seconds including model loads and generation. Provider fees were $0.
The red result was also visually inspected; that inspection did not change the
scoring rules. Each PNG is bound to a native receipt and verified SHA-256.
The scorer checks the frozen manifest and its own preflight source hash before
measuring pixels. [Result](evidence/image-weight-baseline-2026-11-14.json).

**Decision: skip training on this saturated baseline.** No image weights changed
and no adapter was produced. A more difficult prospective benchmark must expose
a baseline weakness before training can establish an improvement. Actual media
training, post-change evaluation, saved-adapter reload and promotion integration
remain incomplete. The text trainer and promotion gate do not admit media adapters.

Recheck the retained baseline without generation:

```sh
.venv-media/bin/python scripts/evaluate_image_weight_baseline.py
```

The native baseline probe is create-only and refuses to overwrite an existing or
partial baseline. It must not be rerun blindly. The two ordinary media integration
tests and two geometry tests passed; the actual four-image generation probe passed
separately. Neither file validity nor this simple geometric benchmark establishes
general image quality, image editing, audio learning or video capability.

## Counting trial: actual image adapter training

The next frozen benchmark requested four, five, six and seven separate green
squares. Deterministic connected-component checks measured count, component
shape, foreground area and white background. Eighteen owned training references
covered counts two through seven in red, green and blue. Test seeds were fixed
before generation; the concepts themselves overlap training. This is not a test
of novel concepts or general visual quality.

The native sandboxed baseline passed **2/4**: requests for five and seven yielded
four and nine squares respectively. The four- and six-square cases passed.
[Baseline evidence](evidence/image-count-baseline-2026-11-14.json).

`scripts/run_image_weight_training.py` ran the frozen configuration for **90
steps**, writing a separate rank-four adapter for one query projection. The first
launch failed before optimization because a required library argument was omitted;
that log and failure record were retained. A corrected, separately numbered launch
completed in **167.96 seconds**. Checkpoint tensor payloads changed between steps
12 and 90; the final adapter is 98,618 bytes. The existing pinned base files were
rehash-verified unchanged after training.

The development runner requires the owned office to be stopped and acquires the
existing cross-process media lock. It clears the child environment and uses a
default-deny macOS sandbox: model files are read-only, writes are limited to the
experiment and Metal compiler cache, and network access is not granted. The run
has a 30-minute subprocess timeout. The MLX memory setting is a working-set guide,
not a hard GPU quota. This is an owner-invoked experimental runner, not an MCP tool
or a production training/promotion service.

`scripts/evaluate_image_weight_candidate.py` reloaded only the final 90-step
adapter in a **fresh sandboxed process**, then repeated the four fixed seeds.
It took **13.95 seconds**. The result remained **2/4**, with the same counts:
4, 4, 6, 9. No new passes and no observed pass-to-fail regressions. Candidate PNG
hashes were rechecked against render records. Provider fees were $0 throughout;
electricity was not measured. The office was restored after GPU work.

**Decision: no measured task gain; do not promote.**
[Combined evidence](evidence/image-count-weight-trial-2026-11-14.json).
Actual weight updates and successful adapter loading do not establish better
image generation. Audio/video training and general image-quality qualification
remain incomplete. Three image-evaluation tests and the two ordinary native media
tests passed; the four-image native baseline ran separately and passed its
generation/receipt assertions, which are distinct from semantic scores.

## Audio checkpoint acquisition and restricted conversion

The public [Piper checkpoint repository](https://huggingface.co/datasets/rhasspy/piper-checkpoints/tree/005db054ce8f013d8213b7dd4b0917de3c9347fe/en/en_US/ljspeech/high)
contains a training checkpoint for LJ Speech. Its revision, exact byte count and
LFS SHA-256 are frozen in `config/audio-training-source.json`. The 998,167,044-byte
checkpoint was downloaded without credentials into ignored model quarantine,
along with pinned metadata. The model card identifies LJ Speech; the
[dataset publisher](https://keithito.com/LJ-Speech-Dataset/) describes the recordings
and transcripts as public domain in the US. Neither that statement nor repository
license metadata proves every deployment's compliance or that this checkpoint is
identical to the installed ONNX model.

Inspection of the installed Piper training code found warm-start calls using
`torch.load(..., weights_only=False)`. Those paths were not invoked. The owned
converter statically inspects requested pickle globals, permits only the known
`pathlib.PosixPath` data class beyond PyTorch's defaults, and always loads with
`weights_only=True` on CPU. Unknown globals, wrong hashes, non-tensor state entries
and nonfinite floating tensors are rejected. There is no unrestricted fallback.

Actual conversion ran in a default-deny macOS sandbox with no network or private
file access, checkpoint read-only access, and writes limited to its output area.
It saved **949 tensors** (generator and discriminator) into a **332,440,136-byte
Safetensors file**. Optimizer state was discarded. The converted file's SHA-256
was independently rechecked. A fixture test verified tensor preservation and
rejection of mismatched hashes and an unapproved pickle global.
[Conversion evidence](evidence/audio-checkpoint-conversion-2026-11-14.json).

This is **serialization conversion, not training or improved speech**. The
checkpoint stays quarantined and is not admitted to the daily production model
roster. The working ONNX voice is unchanged. Local PyTorch reports MPS available,
but Piper training on MPS, architecture parity, reconstruction from Safetensors,
paired-data preparation, objective intelligibility checks and listening evaluation
remain unverified. No training dependencies were added to the working media
environment. Provider fees were $0; network transfer and local storage were used.

## Audio reconstruction and numerical baseline comparison

The next native probe reconstructed the VITS generator from the converted
Safetensors file using architecture parameters read through the restricted loader.
It loaded **838 generator tensors strictly**, with no missing/unexpected parameters
or shape mismatches. The discriminator tensors were not needed for inference.
It bypassed Lightning checkpoint loading and Piper's unrestricted warm-start code.

On CPU, the fixed sentence “The local company is ready.” produced a finite mono
22,050 Hz waveform: **37,888 samples / 1.718 seconds**. Its peak amplitude was
0.66935. Inference took 0.188 seconds, excluding model loading and setup. The WAV
hash and sample metadata were independently rechecked. A finite waveform alone
does not prove intelligibility or quality.
[Reconstruction evidence](evidence/audio-reconstruction-2026-11-14.json).

A separate sandboxed probe compared the reconstructed generator with the pinned
production ONNX voice on three fixed sentences. Both received identical phoneme
IDs, noise scales of zero and length scale one. Phoneme maps and voice settings
were checked for agreement before synthesis. The criteria, fixed before running,
required matching sample lengths, maximum absolute error <= 0.0001, and RMSE
<= 0.00001.

All **3/3 comparisons passed**. Maximum absolute errors ranged from 0.0000215 to
0.0000562; RMSE ranged from 0.00000125 to 0.00000186. These results support baseline
compatibility on those inputs, not universal numerical equivalence, stochastic
generation parity or better speech.
[Parity evidence](evidence/audio-voice-parity-2026-11-14.json).

No weights changed, no model was admitted or promoted, and provider fees remained
$0. The office stayed running throughout this CPU-only work. Local paired-data
preparation, gradient/training compatibility, prospective intelligibility and
quality benchmarks, and before/after audio learning remain outstanding. The
reconstruction's architecture JSON now permits further work directly from
Safetensors without repeated checkpoint deserialization.

## Paired speech preparation and prospective timing experiment

The LJ Speech 1.1 archive was downloaded from its publisher and verified against
the byte count and SHA-256 published in TensorFlow Datasets. A deterministic
hash ordering selected 64 paired clips: **48 training, 8 validation, 8 test**.
Only selected regular WAV members were copied, using bounded reads and generated
destination names. Links were rejected; no archive paths were extracted directly.
All 64 saved WAV hashes, sample counts and sample rates were independently checked.
[Preparation evidence](evidence/audio-paired-preparation-2026-11-14.json).

These splits isolate the upcoming fine-tuning experiment. LJ Speech was likely
used for original voice training, so they are **not unseen pretraining data**.
The paired manifest retains normalized transcripts and immutable audio hashes.

The installed Piper package lacks its training alignment extension. An independent
CPU dynamic-programming alignment implementation now matches exhaustive optimal
paths for small seeded examples. Five audio conversion, dataset-safety and
alignment tests pass. This establishes a tested primitive, not integrated training
compatibility or improved speech.

`config/audio-weight-timing-experiment.json` freezes a narrow next experiment:
train only the duration predictor for two epochs and compare reference-duration
errors on held-out clips before/after saving and reloading. Timing improvement
alone cannot qualify a production voice. The protocol was frozen before the run
documented below; its original prospective status is retained as part of that input.
No weights changed or models were promoted during preparation; provider fees
were $0 and the local office remained healthy.

## Native speech duration weight experiment

The fixed two-epoch run completed **96 optimizer steps in 18.22 seconds** on CPU.
Only the stochastic duration predictor was trainable; **284 saved tensors changed**.
The original Safetensors hash stayed unchanged. Baseline results were persisted
before optimization. Alignment targets came only from the 48 training clips.
No checkpoint was selected using the test split.

| Fine-tuning split | Baseline mean absolute relative duration error | Candidate | Cases improved / unchanged / worse |
| --- | --- | --- | --- |
| Validation (8 clips) | 8.816% | 8.133% | 7 / 0 / 1 |
| Test (8 clips) | 8.557% | 7.829% | 6 / 1 / 1 |

The test average fell **8.51% relative to baseline**. The worse test clip's error
increased by 0.454 percentage points; the worse validation clip increased by 1.208
points. Both splits passed the experiment's predefined aggregate and tolerance
criteria, but **individual regressions remain**. The candidate stays quarantined
and is not eligible for a claim of regression-free improvement.

A freshly constructed duration module loaded the saved candidate and reproduced
all timing predictions exactly. A separate sandboxed process then loaded full
baseline and candidate generators and synthesized all eight test sentences with
each: **16 WAV outputs**, finite signals, no clipped samples, and sample lengths
matching the reported predictions. Saved WAV hashes and metadata were independently
verified. These checks do not measure intelligibility, pronunciation or naturalness.

[Source-bound timing and synthesis evidence](evidence/audio-timing-weight-trial-2026-11-14.json).
Private native reports, complete per-case results, saved weights, sandbox profiles,
executed source snapshots and WAVs are retained under
`data/model-quarantine/piper-ljspeech-training/conversion-attempt/timing-experiment-v1`.
Five preparation/conversion/alignment tests still pass. The office remained healthy;
provider fees were $0. General audio quality, fresh-domain evaluation and production
admission remain unqualified.

## Strict regression gate and weight interpolation

A reusable timing gate now recomputes duration errors from reference/predicted
seconds, rejects empty, mismatched, duplicate or nonfinite cases, and rejects
**any individual increase in error**, even when the average improves. Applying it
to the original timing candidate rejects both its validation and test results.
This stricter admission decision does not rewrite the earlier experimental
tolerance criteria or its retained measurements. Eight audio tests pass, including
a case proving that an average gain cannot hide a regression.

A validation-only weight-space search then saved and loaded three duration
predictors using `base + alpha * (candidate - base)`. The scales were fixed before
evaluation. Other generator weights were unchanged.

| Candidate delta scale | Validation mean duration error | Strict gate |
| --- | --- | --- |
| 0 (original base) | 8.816% | Baseline |
| 0.25 | 8.555% | Rejected: one regression |
| 0.50 | 8.425% | Rejected: one regression |
| 0.75 | 8.257% | Rejected: one regression |

All three worsened `LJ034-0079`. No candidate was selected, no interpolation was
evaluated on test clips, and no model was promoted. Fresh qualification is therefore
not warranted for these candidates. Saved candidate hashes and gate calculations
were independently checked; the base hash remained unchanged. Provider fees were
$0, and the local office stayed healthy.
[Interpolation evidence](evidence/audio-weight-interpolation-2026-11-14.json).
