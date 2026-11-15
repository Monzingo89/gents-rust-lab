//! Local inference receives platform essentials, never company/provider secrets.
use std::ffi::OsString;

pub fn allowed(
    values: impl IntoIterator<Item = (OsString, OsString)>,
) -> Vec<(OsString, OsString)> {
    const NAMES: &[&str] = &[
        "PATH",
        "LANG",
        "LC_ALL",
        "TMPDIR",
        "TMP",
        "TEMP",
        "SYSTEMROOT",
        "WINDIR",
        "CUDA_VISIBLE_DEVICES",
        "HIP_VISIBLE_DEVICES",
        "ROCR_VISIBLE_DEVICES",
        "ONEAPI_DEVICE_SELECTOR",
        "ZE_AFFINITY_MASK",
        "GGML_METAL_PATH_RESOURCES",
    ];
    values
        .into_iter()
        .filter(|(name, _)| NAMES.contains(&name.to_string_lossy().to_ascii_uppercase().as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_platform_and_device_selection_without_credentials_or_remote_routes() {
        let input = [
            ("PATH", "platform"),
            ("SystemRoot", "windows"),
            ("CUDA_VISIBLE_DEVICES", "0"),
            ("HOME", "private-home"),
            ("HF_TOKEN", "private"),
            ("OPENAI_API_KEY", "private"),
            ("DATABRICKS_TOKEN", "private"),
            ("AZURE_CLIENT_SECRET", "private"),
            ("LLAMA_ARG_MODEL_URL", "remote"),
            ("HTTP_PROXY", "remote"),
            ("PYTHONPATH", "unreviewed modules"),
        ];
        let result = allowed(input.into_iter().map(|(k, v)| (k.into(), v.into())));
        assert_eq!(result.len(), 3);
        assert!(result
            .iter()
            .all(|(_, value)| value != "private" && value != "remote" && value != "private-home"));
    }
}
