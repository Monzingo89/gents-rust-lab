//! Pure bounded WAV PCM16 measurements. No file access, playback, metadata text,
//! models, transcription, clinical judgment or executable codec/plugin loading.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
fn u16le(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}
fn u32le(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}
pub fn inspect_pcm16(bytes: &[u8]) -> Result<Value> {
    ensure!(
        (12..=MAX_BYTES).contains(&bytes.len()),
        "WAV input outside complete-file byte bound"
    );
    ensure!(
        &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE",
        "require little-endian RIFF WAVE; RF64/RIFX and other containers unsupported"
    );
    ensure!(
        u32le(bytes, 4) as u64 + 8 == bytes.len() as u64,
        "RIFF declared length differs from complete source length"
    );
    let (mut format, mut data) = (None, None);
    let (mut at, mut chunks) = (12usize, 0usize);
    while at < bytes.len() {
        chunks += 1;
        ensure!(
            chunks <= 128 && bytes.len() - at >= 8,
            "WAV chunk count/header bound exceeded"
        );
        let size = u32le(bytes, at + 4) as usize;
        let start = at + 8;
        let end = start
            .checked_add(size)
            .context("WAV chunk length overflow")?;
        let padded = end.checked_add(size % 2).context("WAV padding overflow")?;
        ensure!(
            padded <= bytes.len(),
            "WAV chunk/padding exceeds complete source"
        );
        match &bytes[at..at + 4] {
            b"fmt " => {
                ensure!(
                    format.is_none()
                        && (size == 16 || (size == 18 && u16le(bytes, start + 16) == 0)),
                    "duplicate or unsupported WAV format chunk"
                );
                format = Some((start, size));
            }
            b"data" => {
                ensure!(data.is_none(), "duplicate WAV data chunk");
                data = Some((start, end));
            }
            _ => {} // Unknown payload/metadata is neither parsed nor echoed.
        }
        at = padded;
    }
    let (f, _) = format.context("WAV format chunk missing")?;
    let (start, end) = data.context("WAV data chunk missing")?;
    let channels = u16le(bytes, f + 2) as usize;
    let rate = u32le(bytes, f + 4);
    let align = u16le(bytes, f + 12) as usize;
    ensure!(
        u16le(bytes, f) == 1 && [1, 2].contains(&channels) && u16le(bytes, f + 14) == 16,
        "only mono/stereo integer PCM16 supported; no compressed, float or extensible codec"
    );
    ensure!(
        rate > 0
            && align == channels * 2
            && u32le(bytes, f + 8) as u64 == rate as u64 * align as u64,
        "inconsistent WAV sample rate, byte rate or block alignment"
    );
    ensure!(
        (end - start) % align == 0,
        "WAV data ends inside an audio frame"
    );
    let frames = (end - start) / align;
    let mut statistics = Vec::new();
    for channel in 0..channels {
        let (mut minimum, mut maximum) = (None::<i16>, None::<i16>);
        let (mut sum, mut zeros, mut rails, mut changes) = (0i64, 0usize, 0usize, 0usize);
        let mut previous = None::<i16>;
        for frame in 0..frames {
            let offset = start + frame * align + channel * 2;
            let sample = i16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
            minimum = Some(minimum.map_or(sample, |v| v.min(sample)));
            maximum = Some(maximum.map_or(sample, |v| v.max(sample)));
            sum += sample as i64;
            zeros += usize::from(sample == 0);
            rails += usize::from(sample == i16::MIN || sample == i16::MAX);
            if previous.is_some_and(|p| (p < 0 && sample > 0) || (p > 0 && sample < 0)) {
                changes += 1;
            }
            previous = Some(sample);
        }
        let peak = minimum
            .map_or(0, |v| (v as i32).abs())
            .max(maximum.map_or(0, |v| (v as i32).abs()));
        statistics.push(json!({"channel":channel,"minimum":minimum,"maximum":maximum,"peak_abs":peak,"sample_sum":sum,"zero_samples":zeros,"rail_samples":rails,"adjacent_sign_changes":changes}));
    }
    Ok(
        json!({"format":"wav_pcm16_le","channels":channels,"sample_rate_hz":rate,"bits_per_sample":16,"frame_count":frames,
        "duration_seconds":{"numerator":frames,"denominator":rate},"channel_statistics":statistics,"all_samples_zero":statistics.iter().all(|s|s["zero_samples"]==frames),
        "measurement_scope":"Exact decoded sample integers and rational duration. Sign changes require adjacent nonzero samples; zero breaks adjacency. Rail hits do not prove clipping. No content, speech, identity, diagnosis or compliance inference."}),
    )
}

#[cfg(test)]
#[path = "../tests/support/audio.rs"]
mod fixture;
#[cfg(test)]
mod tests {
    use super::*;
    use fixture::*;
    #[test]
    fn signed_extrema_zero_adjacency_and_exact_duration() {
        let value = inspect_pcm16(&wav(1, 8000, &[-32768, -1, 0, 1, 32767, -2, 2])).unwrap();
        assert_eq!(value["frame_count"], 7);
        assert_eq!(
            value["duration_seconds"],
            json!({"numerator":7,"denominator":8000})
        );
        assert_eq!(
            value["channel_statistics"][0],
            json!({"channel":0,"minimum":-32768,"maximum":32767,"peak_abs":32768,"sample_sum":-1,"zero_samples":1,"rail_samples":2,"adjacent_sign_changes":2})
        );
        assert_eq!(value["all_samples_zero"], false);
    }
    #[test]
    fn stereo_interleaving_and_empty_samples_are_distinct_from_speech() {
        let value = inspect_pcm16(&wav(2, 48000, &[1, -1, -1, 1, 0, 0, 2, -2])).unwrap();
        assert_eq!(value["frame_count"], 4);
        assert_eq!(value["channel_statistics"][0]["sample_sum"], 2);
        assert_eq!(value["channel_statistics"][1]["sample_sum"], -2);
        assert_eq!(value["channel_statistics"][0]["adjacent_sign_changes"], 1);
        let empty = inspect_pcm16(&wav(1, 1, &[])).unwrap();
        assert_eq!(empty["frame_count"], 0);
        assert!(empty["channel_statistics"][0]["minimum"].is_null());
        assert_eq!(empty["all_samples_zero"], true);
        let silent = inspect_pcm16(&wav(1, 8000, &[0; 4])).unwrap();
        assert_eq!(silent["all_samples_zero"], true);
        assert_eq!(silent["channel_statistics"][0]["zero_samples"], 4);
        assert!(silent.get("transcript").is_none());
    }
    #[test]
    fn unknown_metadata_padding_and_data_order_do_not_leak_payload() {
        let fmt = chunk(b"fmt ", &format(1, 8000));
        let data = chunk(b"data", &[1, 0, 255, 255]);
        let metadata = chunk(b"LIST", b"PRIVATE_METADATA_NOT_SPEECH");
        let value = inspect_pcm16(&riff(&[metadata, data, fmt])).unwrap();
        assert_eq!(value["frame_count"], 2);
        assert!(!value.to_string().contains("PRIVATE_METADATA"));
        let mut extended = format(1, 8000);
        extended.extend_from_slice(&0u16.to_le_bytes());
        assert!(inspect_pcm16(&riff(&[chunk(b"fmt ", &extended), chunk(b"data", &[])])).is_ok());
    }
    #[test]
    fn malformed_unsupported_ambiguous_and_partial_inputs_fail_closed() {
        let valid = wav(1, 8000, &[-1, 0, 1]);
        for length in 0..valid.len() {
            let mut prefix = valid[..length].to_vec();
            if length >= 8 {
                prefix[4..8].copy_from_slice(&((length - 8) as u32).to_le_bytes());
            }
            assert!(inspect_pcm16(&prefix).is_err(), "prefix {length}");
        }
        for (offset, data) in [
            (0, b"RIFX".to_vec()),
            (20, 3u16.to_le_bytes().to_vec()),
            (22, 3u16.to_le_bytes().to_vec()),
            (24, 0u32.to_le_bytes().to_vec()),
            (28, 1u32.to_le_bytes().to_vec()),
            (32, 1u16.to_le_bytes().to_vec()),
            (34, 8u16.to_le_bytes().to_vec()),
            (40, u32::MAX.to_le_bytes().to_vec()),
        ] {
            let mut bad = valid.clone();
            bad[offset..offset + data.len()].copy_from_slice(&data);
            assert!(inspect_pcm16(&bad).is_err());
        }
        let fmt = chunk(b"fmt ", &format(1, 8000));
        let data = chunk(b"data", &[]);
        for chunks in [
            vec![fmt.clone(), fmt.clone(), data.clone()],
            vec![fmt.clone(), data.clone(), data.clone()],
            vec![fmt],
            vec![data],
            vec![chunk(b"fmt ", &format(2, 8000)), chunk(b"data", &[1, 0])],
        ] {
            assert!(inspect_pcm16(&riff(&chunks)).is_err());
        }
        assert!(inspect_pcm16(&vec![0; MAX_BYTES + 1]).is_err());
    }
    #[test]
    fn chunk_budget_and_missing_odd_padding_are_rejected_without_identifier_echo() {
        let mut chunks = vec![chunk(b"fmt ", &format(1, 8000))];
        for _ in 0..128 {
            chunks.push(chunk(b"JUNK", &[]));
        }
        chunks.push(chunk(b"data", &[]));
        assert!(inspect_pcm16(&riff(&chunks)).is_err());
        let mut bytes = riff(&[
            chunk(b"fmt ", &format(1, 8000)),
            chunk(b"data", &[]),
            chunk(b"PRIV", b"x"),
        ]);
        bytes.pop();
        let len = bytes.len();
        bytes[4..8].copy_from_slice(&((len - 8) as u32).to_le_bytes());
        let error = inspect_pcm16(&bytes).unwrap_err().to_string();
        assert!(!error.contains("PRIV"));
    }
    #[test]
    fn maximum_file_handles_negative_full_scale_without_overflow() {
        let mut data = vec![0u8; MAX_BYTES - 44];
        for sample in data.as_chunks_mut::<2>().0 {
            sample.copy_from_slice(&i16::MIN.to_le_bytes());
        }
        let bytes = riff(&[chunk(b"fmt ", &format(1, 8000)), chunk(b"data", &data)]);
        assert_eq!(bytes.len(), MAX_BYTES);
        let measured = inspect_pcm16(&bytes).unwrap();
        let frames = data.len() / 2;
        assert_eq!(measured["frame_count"], frames);
        assert_eq!(measured["channel_statistics"][0]["rail_samples"], frames);
        assert_eq!(
            measured["channel_statistics"][0]["sample_sum"],
            -32768i64 * frames as i64
        );
        assert_eq!(measured["channel_statistics"][0]["peak_abs"], 32768);
    }
}
