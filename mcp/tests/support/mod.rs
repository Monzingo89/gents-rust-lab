#![allow(dead_code)]
pub mod audio;
use gents_local_mcp::{
    authority::{Authority, SqliteAuthority},
    governance::{self, Policy},
    Phase, Scope,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::PathBuf;
pub struct Fixture {
    pub root: PathBuf,
    pub scope: Scope,
    pub authority: SqliteAuthority,
}
impl Fixture {
    pub fn new() -> Self {
        let root = std::env::temp_dir().join(format!("gents-mcp-test-{}", uuid::Uuid::new_v4()));
        let company = "fixture";
        let folder = root.join("data/companies").join(company);
        std::fs::create_dir_all(&folder).unwrap();
        let scope = Scope {
            company: company.into(),
            owner: "local-operator".into(),
            actor: "Worker".into(),
            phase: Phase::Worker,
            expected_revision: None,
            run: uuid::Uuid::new_v4(),
        };
        let db = Connection::open(folder.join("brain.sqlite3")).unwrap();
        db.execute_batch("CREATE TABLE company(owner TEXT,name TEXT);CREATE TABLE jobs(run_id TEXT PRIMARY KEY,digest TEXT,approval TEXT,state TEXT,intake TEXT DEFAULT '{}');CREATE TABLE audit(seq INTEGER PRIMARY KEY,run_id TEXT,kind TEXT,body TEXT,created INTEGER);CREATE TABLE run_governance(run_id TEXT PRIMARY KEY,revision INTEGER,manifest TEXT);").unwrap();
        db.execute("INSERT INTO company VALUES(?1,'fixture')", [&scope.owner])
            .unwrap();
        db.execute(
            "INSERT INTO jobs(run_id,digest,approval,state) VALUES(?1,'frozen',?2,'approved')",
            params![
                scope.run.to_string(),
                json!({"owner":scope.owner,"sha256":"frozen","type":"local_operator_approval"})
                    .to_string()
            ],
        )
        .unwrap();
        let access =
            |tools: Value, senders: Value| json!({"tools":tools,"skills":[],"memory_from":senders});
        let grants = json!({"Worker":access(json!(["integer_arithmetic","memory_append","memory_search"]),json!([])),"Reader":access(json!(["memory_search"]),json!(["Worker"])),"Outsider":access(json!(["memory_append","memory_search"]),json!([]))});
        let policy: Policy = serde_json::from_value(
            json!({"revision":1,"master":"Worker","ceilings":grants,"grants":grants}),
        )
        .unwrap();
        governance::save(&db, &scope.run.to_string(), &policy, 0).unwrap();
        Self {
            authority: SqliteAuthority::new(root.clone()).unwrap(),
            root,
            scope,
        }
    }
    pub fn actor(&self, name: &str) -> Scope {
        Scope {
            actor: name.into(),
            ..self.scope.clone()
        }
    }
    pub fn db(&self) -> Connection {
        self.authority.open(&self.scope).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
pub fn install_files(f: &Fixture) {
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let prior = policy.revision;
    policy.revision += 1;
    for access in [&mut policy.ceilings, &mut policy.grants] {
        access.get_mut("Worker").unwrap().tools.extend([
            "files:read".into(),
            "files:write".into(),
            "files:list".into(),
        ]);
        access
            .get_mut("Reader")
            .unwrap()
            .tools
            .extend(["files:read".into(), "files:list".into()]);
    }
    governance::save(&db, &f.scope.run.to_string(), &policy, prior).unwrap();
}
