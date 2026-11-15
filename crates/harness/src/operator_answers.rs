//! Authenticated operator input is canonical, not part of a worker's mutable
//! checkpoint. Only runtime-authenticated callers may publish these receipts.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    version: u32,
    company: String,
    run_id: Uuid,
    question_id: String,
    question_sha256: String,
    pub owner: String,
    pub answer: String,
    pub answered_at_ms: u64,
}

pub trait AnswerRepository {
    fn get(&self, question: &Value) -> Result<Option<Answer>>;
    fn record(&self, question: &Value, owner: &str, answer: &str, now: u64) -> Result<()>;
}

pub struct FileAnswers<'a> {
    pub root: &'a Path,
    pub company: &'a str,
    pub run: Uuid,
}
impl FileAnswers<'_> {
    fn key(&self, question: &Value) -> Result<(PathBuf, String)> {
        let id = question["id"]
            .as_str()
            .context("question identity missing")?;
        let text = question["question"]
            .as_str()
            .context("question text missing")?;
        anyhow::ensure!(
            !id.is_empty() && id.len() <= 200 && !text.is_empty() && text.len() <= 100_000,
            "question binding must be bounded"
        );
        let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
        let name = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(
                &json!({"company":self.company,"run":self.run,"id":id,"question_sha256":hash})
            )?)
        );
        let directory = crate::company::scoped(
            self.root,
            self.company,
            &["runs", &self.run.to_string(), "operator-answers"],
        )?;
        Ok((directory.join(format!("{name}.json")), hash))
    }
}
impl AnswerRepository for FileAnswers<'_> {
    fn get(&self, question: &Value) -> Result<Option<Answer>> {
        let (path, hash) = self.key(question)?;
        anyhow::ensure!(!path.is_symlink(), "operator answer symlink rejected");
        if !path.exists() {
            return Ok(None);
        }
        anyhow::ensure!(
            path.metadata()?.len() <= 100_000,
            "operator receipt exceeds bound"
        );
        let receipt: Answer = serde_json::from_slice(&std::fs::read(path)?)?;
        anyhow::ensure!(
            receipt.version == 1
                && receipt.company == self.company
                && receipt.run_id == self.run
                && question["id"] == receipt.question_id
                && receipt.question_sha256 == hash
                && !receipt.owner.is_empty()
                && receipt.owner.len() <= 200
                && !receipt.answer.trim().is_empty()
                && receipt.answer.len() <= 10000,
            "operator receipt scope or content changed"
        );
        Ok(Some(receipt))
    }
    fn record(&self, question: &Value, owner: &str, answer: &str, now: u64) -> Result<()> {
        anyhow::ensure!(
            !owner.is_empty()
                && owner.len() <= 200
                && !answer.trim().is_empty()
                && answer.len() <= 10000,
            "operator answer must be bounded"
        );
        anyhow::ensure!(self.get(question)?.is_none(), "question already answered");
        let (target, hash) = self.key(question)?;
        let directory = target.parent().context("operator answer parent missing")?;
        std::fs::create_dir_all(directory)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))?;
        }
        let receipt = Answer {
            version: 1,
            company: self.company.into(),
            run_id: self.run,
            question_id: question["id"].as_str().unwrap().into(),
            question_sha256: hash,
            owner: owner.into(),
            answer: answer.into(),
            answered_at_ms: now,
        };
        let temporary = directory.join(format!(".{}.tmp", Uuid::new_v4()));
        let result = (|| -> Result<()> {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let file = options.open(&temporary)?;
            serde_json::to_writer(&file, &receipt)?;
            file.sync_all()?;
            // Publish a complete synced receipt, without replacing a concurrent
            // winner. A crash can leave a harmless private temporary hard link.
            std::fs::hard_link(&temporary, &target)
                .context("operator answer already published or publication failed")?;
            std::fs::File::open(directory)?.sync_all()?;
            Ok(())
        })();
        let _ = std::fs::remove_file(temporary);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipts_are_immutable_scope_bound_private_and_not_replayed_for_new_questions() {
        let root = std::env::temp_dir().join(format!("gents-operator-receipts-{}", Uuid::new_v4()));
        let run = Uuid::new_v4();
        let repo = FileAnswers {
            root: &root,
            company: "one",
            run,
        };
        let question = json!({"id":"question-one","question":"Which project?","answer":null});
        repo.record(&question, "owner-fixture", "Owner Project", 42)
            .unwrap();
        let receipt = repo.get(&question).unwrap().unwrap();
        assert_eq!(receipt.answer, "Owner Project");
        assert_eq!(receipt.owner, "owner-fixture");
        assert!(repo
            .record(&question, "other-fixture", "replacement", 43)
            .is_err());
        let changed = json!({"id":"question-one","question":"Approve a different action?"});
        assert!(repo.get(&changed).unwrap().is_none());
        assert!(FileAnswers {
            root: &root,
            company: "two",
            run
        }
        .get(&question)
        .unwrap()
        .is_none());
        assert!(FileAnswers {
            root: &root,
            company: "one",
            run: Uuid::new_v4()
        }
        .get(&question)
        .unwrap()
        .is_none());
        let concurrent = json!({"id":"question-race","question":"Choose one project?"});
        let barrier = std::sync::Barrier::new(2);
        let winners = std::thread::scope(|scope| {
            let first = scope.spawn(|| {
                barrier.wait();
                repo.record(&concurrent, "first-owner", "First", 43).is_ok()
            });
            let second = scope.spawn(|| {
                barrier.wait();
                repo.record(&concurrent, "second-owner", "Second", 44)
                    .is_ok()
            });
            usize::from(first.join().unwrap()) + usize::from(second.join().unwrap())
        });
        assert_eq!(
            winners, 1,
            "concurrent replies must never replace each other"
        );
        assert!(matches!(
            repo.get(&concurrent).unwrap().unwrap().answer.as_str(),
            "First" | "Second"
        ));
        let (path, _) = repo.key(&question).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o600);
        }
        let mut forged: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        forged["run_id"] = json!(Uuid::new_v4());
        std::fs::write(&path, serde_json::to_vec(&forged).unwrap()).unwrap();
        assert!(repo.get(&question).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
