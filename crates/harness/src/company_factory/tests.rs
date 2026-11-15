use super::*;
use std::collections::BTreeMap;

struct Repositories {
    architectures: Vec<Architecture>,
    agents: BTreeMap<String, Agent>,
    tools: BTreeMap<String, Tool>,
    profiles: BTreeMap<String, ControlProfile>,
}
impl ArchitectureRepository for Repositories {
    fn architectures(&self) -> Result<Vec<Architecture>> {
        Ok(self.architectures.clone())
    }
}
impl AgentRepository for Repositories {
    fn agent(&self, id: &str) -> Result<Option<Agent>> {
        Ok(self.agents.get(id).cloned())
    }
}
impl ToolRepository for Repositories {
    fn tool(&self, id: &str) -> Result<Option<Tool>> {
        Ok(self.tools.get(id).cloned())
    }
}
impl ComplianceProfileRepository for Repositories {
    fn control_profile(&self, id: &str) -> Result<Option<ControlProfile>> {
        Ok(self.profiles.get(id).cloned())
    }
}
pub(super) fn config() -> crate::Config {
    let model = json!({"provider":"local","model":"fixture-not-installed","path":"models/not-installed.gguf"});
    serde_json::from_value(json!({"company_mode":true,"company_tools":true,"agents":[{"id":"Worker","assignment":"Fixture only","planner":model,"worker":model}],"max_steps":1,"max_tokens":256})).unwrap()
}
pub(super) fn request() -> PreviewRequest {
    serde_json::from_value(json!({"expected_revision":2,"specification":{"version":1,"objective":"Read the owner's public fixture","inputs":["fixture.txt"],"outputs":["A receipt-bound read result"],"definition_of_done":["Native read matches the frozen input"],"exclusions":["No network or paid services"],"coordination":"ordered","data_classification":"synthetic","control_profiles":["local_baseline"],"required_capabilities":[]}})).unwrap()
}
pub(super) fn session() -> Value {
    json!({"status":"ready","revision":2,"proposal":{"status":"ready","question":"","definition_of_done":["Native read matches the frozen input"],"assignments":[{"actor":"Worker","task":"Read fixture","tools":["files:read"],"acceptance":["Native read matches input"]}]}})
}
fn repositories() -> Repositories {
    let config = config();
    let native = repositories::InstalledRepositories::new(&config).unwrap();
    Repositories {
        architectures: native.architectures().unwrap(),
        agents: BTreeMap::from([("Worker".into(), native.agent("Worker").unwrap().unwrap())]),
        tools: BTreeMap::from([(
            "files:read".into(),
            native.tool("files:read").unwrap().unwrap(),
        )]),
        profiles: BTreeMap::from([(
            "local_baseline".into(),
            native.control_profile("local_baseline").unwrap().unwrap(),
        )]),
    }
}
fn compose(
    repositories: &Repositories,
    request: &PreviewRequest,
    session: &Value,
) -> Result<Blueprint> {
    CompanyFactory {
        architectures: repositories,
        agents: repositories,
        tools: repositories,
        compliance: repositories,
    }
    .compose(request, session)
}
fn codes(blueprint: &Blueprint) -> BTreeSet<&str> {
    blueprint.gaps.iter().map(|gap| gap.code.as_str()).collect()
}

#[test]
fn actual_flow_is_review_only_and_other_patterns_cannot_silently_use_it() {
    let repos = repositories();
    let mut request = request();
    let blueprint = compose(&repos, &request, &session()).unwrap();
    assert_eq!(blueprint.status, "ready_for_owner_review");
    assert_eq!(
        blueprint.architecture.executor.as_deref(),
        Some("autonomy_ordered_team_v1")
    );
    assert!(!blueprint.execution_authorized && blueprint.preview_only);
    assert_eq!(blueprint.tools.len(), 1);
    assert_eq!(blueprint.assignments[0].tools, vec!["files:read"]);
    assert!(blueprint
        .required_controls
        .contains(&"independent_acceptance".into()));
    for (coordination, id) in [
        (Coordination::AdaptiveDelegation, "orchestration"),
        (Coordination::PeerReview, "collaboration"),
    ] {
        request.specification.coordination = coordination;
        let blueprint = compose(&repos, &request, &session()).unwrap();
        assert_eq!(blueprint.architecture.id, id);
        assert!(blueprint.architecture.executor.is_none());
        assert_eq!(
            codes(&blueprint),
            BTreeSet::from(["executor_not_implemented"])
        );
        assert_eq!(blueprint.status, "missing_capability");
    }
}

#[test]
fn missing_owned_tools_and_ineligible_dependencies_are_explicit_gaps() {
    let mut repos = repositories();
    let mut request = request();
    request
        .specification
        .required_capabilities
        .push("not_installed_local_tool".into());
    repos.agents.get_mut("Worker").unwrap().local_native_worker = false;
    let tool = repos.tools.get_mut("files:read").unwrap();
    tool.custody = Custody::External;
    tool.platform_supported = false;
    tool.nontransactional_effects = true;
    tool.local_only = false;
    tool.provider_cost_usd = 1;
    let blueprint = compose(&repos, &request, &session()).unwrap();
    assert_eq!(
        codes(&blueprint),
        BTreeSet::from([
            "owned_tool_missing",
            "native_local_worker_required",
            "tool_custody_not_project_owned",
            "tool_unavailable_on_host",
            "tool_outside_zero_fee_local_scope"
        ])
    );
    assert!(!blueprint.execution_authorized);
    repos.agents.clear();
    repos.tools.clear();
    let blueprint = compose(&repos, &request, &session()).unwrap();
    assert!(codes(&blueprint).contains("agent_not_configured"));
    assert!(codes(&blueprint).contains("owned_tool_missing"));
}

#[test]
fn control_profiles_add_requirements_but_cannot_certify_sensitive_data() {
    let mut repos = repositories();
    repos.profiles.insert(
        "owner_scope".into(),
        ControlProfile {
            id: "owner_scope".into(),
            version: 2,
            required_controls: vec!["owner_defined_control".into()],
            gaps: vec!["evidence_not_supplied".into()],
        },
    );
    let mut request = request();
    request.specification.control_profiles = vec!["owner_scope".into(), "unknown_profile".into()];
    request.specification.data_classification = DataClassification::Regulated;
    let blueprint = compose(&repos, &request, &session()).unwrap();
    assert_eq!(
        codes(&blueprint),
        BTreeSet::from([
            "control_evidence_missing",
            "control_profile_missing",
            "owner_data_scope_and_control_assessment_required"
        ])
    );
    assert!(blueprint
        .required_controls
        .contains(&"owner_defined_control".into()));
    assert!(blueprint
        .required_controls
        .contains(&"current_actor_phase_policy".into()));
    assert_eq!(blueprint.control_profiles.len(), 2);
}

#[test]
fn revision_acceptance_and_ambiguous_repository_changes_fail_closed() {
    let mut repos = repositories();
    let mut request = request();
    request.expected_revision = 1;
    assert!(compose(&repos, &request, &session()).is_err());
    request.expected_revision = 2;
    request.specification.definition_of_done = vec!["Changed definition".into()];
    assert!(compose(&repos, &request, &session()).is_err());
    let request = super::tests::request();
    let mut pending = session();
    pending["status"] = json!("question");
    assert!(compose(&repos, &request, &pending).is_err());
    repos.architectures.push(repos.architectures[0].clone());
    assert!(compose(&repos, &request, &session()).is_err());
    repos.architectures.last_mut().unwrap().id = "other_flow".into();
    assert!(compose(&repos, &request, &session()).is_err());
    repos.architectures.pop();
    repos.agents.get_mut("Worker").unwrap().id = "Different actor".into();
    assert!(compose(&repos, &request, &session()).is_err());
}

#[test]
fn content_digests_detect_repository_and_specification_drift() {
    let mut repos = repositories();
    let mut request = request();
    let original = compose(&repos, &request, &session()).unwrap();
    repos.tools.get_mut("files:read").unwrap().descriptor_sha256 = "b".repeat(64);
    let changed = compose(&repos, &request, &session()).unwrap();
    assert_ne!(
        original.repository_snapshot_sha256,
        changed.repository_snapshot_sha256
    );
    assert_eq!(original.proposal_sha256, changed.proposal_sha256);
    assert_eq!(original.specification_sha256, changed.specification_sha256);
    request
        .specification
        .outputs
        .push("Additional requested output".into());
    let changed_spec = compose(&repos, &request, &session()).unwrap();
    assert_ne!(
        changed.specification_sha256,
        changed_spec.specification_sha256
    );
    assert_eq!(
        changed.repository_snapshot_sha256,
        changed_spec.repository_snapshot_sha256
    );
}

#[test]
fn malformed_specs_and_authority_fields_are_rejected() {
    let repos = repositories();
    let mut input = serde_json::to_value(&request().specification).unwrap();
    input["execution_authorized"] = json!(true);
    assert!(serde_json::from_value::<TaskSpecification>(input).is_err());
    let mut request = request();
    request.specification.inputs.clear();
    assert!(compose(&repos, &request, &session()).is_err());
    request.specification.inputs = vec!["repeated".into(), "repeated".into()];
    assert!(compose(&repos, &request, &session()).is_err());
    request.specification.inputs = vec!["control\ntext".into()];
    assert!(compose(&repos, &request, &session()).is_err());
}

#[test]
fn installed_adapters_restrict_file_operations_and_report_platform_support() {
    let mut config = config();
    let repos = repositories::InstalledRepositories::new(&config).unwrap();
    assert!(repos.tool("files").unwrap().is_none());
    assert!(repos.tool("files:delete").unwrap().is_none());
    assert!(repos.tool("rust_verify:arbitrary").unwrap().is_none());
    assert!(repos.tool("missing").unwrap().is_none());
    assert!(repos.tool("finish").unwrap().is_some());
    assert_eq!(
        repos
            .tool("rust_verify")
            .unwrap()
            .unwrap()
            .platform_supported,
        cfg!(target_os = "macos")
    );
    assert_ne!(
        repos.tool("files:read").unwrap().unwrap().descriptor_sha256,
        repos
            .tool("files:write")
            .unwrap()
            .unwrap()
            .descriptor_sha256
    );
    config.agents[0].worker.provider = "mock".into();
    assert!(
        !repositories::InstalledRepositories::new(&config)
            .unwrap()
            .agent("Worker")
            .unwrap()
            .unwrap()
            .local_native_worker
    );
    config.agents.push(config.agents[0].clone());
    assert!(repositories::InstalledRepositories::new(&config).is_err());
}

#[test]
fn tenant_scoped_preview_never_creates_a_job_or_changes_planning() {
    let root = std::env::temp_dir().join(format!("factory-preview-{}", uuid::Uuid::new_v4()));
    crate::company::operation(
        &root,
        "fixture",
        "alice",
        &json!({"op":"register","name":"Factory fixture"}),
    )
    .unwrap();
    let db = crate::company::connect(&root, "fixture", "alice", false).unwrap();
    db.execute_batch(
        "CREATE TABLE ceo_planning(id TEXT PRIMARY KEY,revision INTEGER,state TEXT,body TEXT)",
    )
    .unwrap();
    let id = uuid::Uuid::new_v4();
    let body = session();
    db.execute(
        "INSERT INTO ceo_planning VALUES(?1,2,'ready',?2)",
        rusqlite::params![id.to_string(), body.to_string()],
    )
    .unwrap();
    let input = json!({"expected_revision":2,"specification":request().specification});
    let counts = ["jobs", "audit", "run_governance"].map(|table| {
        db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap()
    });
    let result =
        repositories::preview(&root, "fixture", "alice", id, &config(), input.clone()).unwrap();
    assert_eq!(result["blueprint"]["status"], "ready_for_owner_review");
    assert_eq!(result["execution_authorized"], false);
    assert!(repositories::preview(&root, "fixture", "bob", id, &config(), input.clone()).is_err());
    assert!(repositories::preview(&root, "missing", "alice", id, &config(), input).is_err());
    assert_eq!(
        crate::ceo_planning::read(&root, "fixture", "alice", id).unwrap(),
        body
    );
    for (table, before) in ["jobs", "audit", "run_governance"].into_iter().zip(counts) {
        assert_eq!(
            db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            before
        );
    }
    drop(db);
    std::fs::remove_dir_all(root).unwrap();
}
