//! Content-bound inputs for independent document review. This module never
//! authors domain content, supplies scores, or grants tool authority.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// A claimed percentage is insufficient: validate arithmetic and bind the
/// rubric to the artifacts being reviewed. The independent judge still has
/// to assess whether each claimed award is supported by actual evidence.
pub fn audit_score(value: &Value, hashes: &BTreeMap<String, String>) -> Result<f64> {
    let rows = value["rubric"]
        .as_array()
        .context("QA needs an explicit rubric array")?;
    if rows.is_empty() || rows.len() > 64 {
        bail!("QA rubric requires 1..64 rows");
    }
    let mut earned = 0.0;
    let mut possible = 0.0;
    let mut criteria = std::collections::BTreeSet::new();
    for row in rows {
        let criterion = row["criterion"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .context("Each rubric row needs a criterion")?;
        if !criteria.insert(criterion.trim().to_lowercase()) {
            bail!("Duplicate rubric criterion");
        }
        let e = row["earned"]
            .as_f64()
            .context("Rubric earned must be numeric")?;
        let p = row["possible"]
            .as_f64()
            .context("Rubric possible must be numeric")?;
        if !e.is_finite() || !p.is_finite() || e < 0.0 || p <= 0.0 || e > p {
            bail!("Invalid rubric points");
        }
        if row["evidence"].as_str().is_none_or(|s| s.trim().is_empty()) {
            bail!("Each rubric row needs concrete evidence");
        }
        earned += e;
        possible += p;
    }
    let calculated = 100.0 * earned / possible;
    let claimed = value["score_percent"]
        .as_f64()
        .context("QA needs score_percent")?;
    if !calculated.is_finite()
        || !(0.0..=100.0).contains(&claimed)
        || (claimed - calculated).abs() > 0.05
    {
        bail!("Claimed score does not match 100 * sum(earned) / sum(possible)");
    }
    if hashes.is_empty()
        || hashes
            .iter()
            .any(|(path, hash)| value["artifact_sha256"][path] != *hash)
    {
        bail!("QA artifact_sha256 must bind every reviewed document and inventory to its current hash (exclude the score file itself)");
    }
    Ok(calculated)
}

pub trait ArtifactEvidenceSource {
    fn packets(&self, artifacts: &[String]) -> Result<Vec<Value>>;
}

pub struct WorkspaceEvidence<'a> {
    pub workspace: &'a Path,
}

fn read_scoped(workspace: &Path, relative: &str) -> Result<Vec<u8>> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|p| !matches!(p, std::path::Component::Normal(_)))
    {
        bail!("Artifact evidence path is outside workspace");
    }
    let mut checked = workspace.to_path_buf();
    for part in path.components() {
        checked.push(part);
        if checked.is_symlink() {
            bail!("Artifact evidence symlink rejected");
        }
    }
    if std::fs::metadata(&checked)?.len() > 2_000_000 {
        bail!("Review input exceeds two MiB; inspect smaller evidence ranges");
    }
    Ok(std::fs::read(checked)?)
}

impl ArtifactEvidenceSource for WorkspaceEvidence<'_> {
    fn packets(&self, artifacts: &[String]) -> Result<Vec<Value>> {
        let mut pdf_pages = BTreeMap::new();
        let mut visual = BTreeMap::new();
        let run = self.workspace.parent().context("Run directory missing")?;
        // Original journals preserve complete inspected content even after the
        // prompt handoff has evicted old optional observations.
        let mut journals = std::fs::read_dir(run)?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| {
                path.file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.starts_with("tool-") && s.ends_with(".json"))
            })
            .collect::<Vec<_>>();
        journals.sort();
        for path in journals {
            if path.is_symlink() || std::fs::metadata(&path)?.len() > 2_000_000 {
                continue;
            }
            let journal: Value = serde_json::from_slice(&std::fs::read(path)?)?;
            let record = &journal["result"];
            if record["result"]["ok"] != true {
                continue;
            }
            let data = &record["result"]["result"];
            match record["action"].as_str() {
                Some("pdf_inspect") => {
                    if let (Some(path), Some(hash)) = (
                        record["arguments"]["path"].as_str(),
                        data["sha256"].as_str(),
                    ) {
                        for page in data["inspected_pages"].as_array().into_iter().flatten() {
                            if let Some(number) = page["page"].as_u64() {
                                pdf_pages.insert(
                                    (path.to_owned(), hash.to_owned(), number),
                                    (data["page_count"].as_u64().unwrap_or(0), page.clone()),
                                );
                            }
                        }
                    }
                }
                Some("image_review") => {
                    for image in data["images"].as_array().into_iter().flatten() {
                        if let Some(hash) = image["sha256"].as_str() {
                            visual.insert(hash.to_owned(),json!({"model":data["model"],"question":data["question"],"observation":data["observation"],"image_sha256":hash,"authority":"Advisory visual findings; independently verify against the task."}));
                        }
                    }
                }
                _ => {}
            }
        }
        let mut packets = Vec::new();
        for artifact in artifacts {
            let relative = Path::new(artifact)
                .strip_prefix(self.workspace)
                .context("Artifact outside workspace")?
                .to_str()
                .context("Artifact path is not UTF-8")?;
            let path = self.workspace.join(relative);
            // PDF bytes can be larger than text review inputs; their current
            // hash is already checked by the artifact gate.
            let hash = sha256(&std::fs::read(&path)?);
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
            {
                let pages = pdf_pages
                    .iter()
                    .filter(|((p, h, _), _)| p == relative && h == &hash)
                    .collect::<Vec<_>>();
                let count = pages.first().map(|(_, (count, _))| *count).unwrap_or(0);
                if count == 0 || pages.len() != count as usize {
                    bail!("PDF content coverage missing at current hash: {relative}");
                }
                for ((_, _, number), (_, page)) in pages {
                    let render = page["render"].as_str().context("PDF render path missing")?;
                    let image_hash = sha256(&read_scoped(self.workspace, render)?);
                    let findings=visual.get(&image_hash).with_context(||format!("Use image_review on output page {number}: {render}; no current hash-bound visual findings"))?;
                    packets.push(json!({"path":relative,"artifact_sha256":hash,"page":number,"page_count":count,"actual_page_text":page["text"],"text_truncated":page["text_truncated"],"visual_findings":findings}));
                }
            } else {
                let text = String::from_utf8(read_scoped(self.workspace, relative)?)
                    .context("Artifact text must be UTF-8")?;
                let chars = text.chars().collect::<Vec<_>>();
                for (index, chunk) in chars.chunks(8000).enumerate() {
                    packets.push(json!({"path":relative,"artifact_sha256":hash,"chunk":index+1,"total_chunks":chars.len().div_ceil(8000),"actual_content":chunk.iter().collect::<String>()}));
                }
            }
        }
        Ok(packets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scores_require_correct_arithmetic_and_current_artifacts() {
        let hashes = BTreeMap::from([("output.pdf".into(), "a".repeat(64))]);
        let mut report = json!({"score_percent":90,"rubric":[{"criterion":"Example criterion","earned":9,"possible":10,"evidence":"page 1"}],"artifact_sha256":hashes});
        assert_eq!(audit_score(&report, &hashes).unwrap(), 90.0);
        report["score_percent"] = json!(99);
        assert!(audit_score(&report, &hashes).is_err());
        report["score_percent"] = json!(90);
        report["artifact_sha256"]["output.pdf"] = json!("stale");
        assert!(audit_score(&report, &hashes).is_err());
        report["artifact_sha256"] = json!(hashes);
        report["rubric"][0]["earned"] = json!(11);
        assert!(audit_score(&report, &hashes).is_err());
        assert!(audit_score(&json!({"score_percent":100}), &hashes).is_err());
    }
    #[test]
    fn packets_read_actual_utf8_content_and_reject_unreviewed_output_images() {
        let run =
            std::env::temp_dir().join(format!("gents-document-review-{}", uuid::Uuid::new_v4()));
        let workspace = run.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let output = workspace.join("output.txt");
        std::fs::write(&output, "é".repeat(9000)).unwrap();
        let source = WorkspaceEvidence {
            workspace: &workspace,
        };
        let packets = source.packets(&[output.display().to_string()]).unwrap();
        assert_eq!(packets.len(), 2);
        assert_eq!(
            packets[1]["actual_content"]
                .as_str()
                .unwrap()
                .chars()
                .count(),
            1000
        );
        let pdf = workspace.join("output.pdf");
        std::fs::write(&pdf, b"fixture PDF bytes").unwrap();
        let hash = sha256(b"fixture PDF bytes");
        std::fs::write(run.join("tool-000001.json"),json!({"result":{"action":"pdf_inspect","arguments":{"path":"output.pdf"},"result":{"ok":true,"result":{"sha256":hash,"page_count":1,"inspected_pages":[{"page":1,"render":"page.png","text":"actual page"}]}}}}).to_string()).unwrap();
        std::fs::write(workspace.join("page.png"), b"fixture pixels").unwrap();
        assert!(source
            .packets(&[pdf.display().to_string()])
            .unwrap_err()
            .to_string()
            .contains("image_review"));
        std::fs::write(run.join("tool-000002.json"),json!({"result":{"action":"image_review","result":{"ok":true,"result":{"images":[{"sha256":sha256(b"fixture pixels")}],"observation":"Uncertain observed diagram"}}}}).to_string()).unwrap();
        assert_eq!(
            source.packets(&[pdf.display().to_string()]).unwrap()[0]["actual_page_text"],
            "actual page"
        );
        std::fs::write(workspace.join("page.png"), b"changed").unwrap();
        assert!(source.packets(&[pdf.display().to_string()]).is_err());
        std::fs::remove_dir_all(run).unwrap();
    }
}
