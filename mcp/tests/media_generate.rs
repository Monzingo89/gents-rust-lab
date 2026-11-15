mod support;
use gents_local_mcp::{authority::Authority, governance, media, workspace, Registry};
use serde_json::{json, Value};
use support::Fixture;

#[test]
#[ignore = "frozen image-weight baseline; local GPU, network-denied sandbox; run once"]
fn frozen_image_weight_baseline() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    std::env::set_current_dir(repo).unwrap();
    let name =
        std::env::var("GENTS_IMAGE_EXPERIMENT").unwrap_or_else(|_| "image-geometry-v1".into());
    assert!(
        matches!(name.as_str(), "image-geometry-v1" | "image-count-v1"),
        "unknown frozen experiment"
    );
    let experiment = repo.join("data/weight-experiments").join(name);
    let bytes = std::fs::read(experiment.join("experiment.json")).unwrap();
    let manifest: Value = serde_json::from_slice(&bytes).unwrap();
    let output = experiment.join("baseline");
    // No automatic retries or replacement of a partially generated baseline.
    std::fs::create_dir(&output).expect("baseline already exists; inspect retained evidence");
    std::fs::write(output.join("frozen-experiment.json"), &bytes).unwrap();
    let fixture = granted();
    let registry = Registry::new();
    let workspace = fixture.authority.workspace(&fixture.scope).unwrap();
    let mut receipts = Vec::new();
    for (index, case) in manifest["heldout"].as_array().unwrap().iter().enumerate() {
        let name = format!("case-{index}.png");
        let started = std::time::Instant::now();
        let reply = registry.call(&fixture.authority, &fixture.scope, "image_generate",
            json!({"prompt":case["prompt"],"path":name,"width":case["width"],"height":case["height"],"seed":case["seed"]})).unwrap();
        assert_eq!(reply["value"]["provider_cost_usd"], 0);
        let image = std::fs::read(workspace.join(&name)).unwrap();
        assert_eq!(
            reply["value"]["artifact"]["sha256"],
            gents_local_mcp::digest(&image)
        );
        std::fs::write(output.join(&name), &image).unwrap();
        receipts.push(
            json!({"case":index,"file":name,"image_sha256":gents_local_mcp::digest(&image),
                            "elapsed_ms":started.elapsed().as_millis(),"native_receipt":reply}),
        );
        std::fs::write(output.join("receipts.json"),serde_json::to_vec_pretty(&json!({
            "experiment_sha256":gents_local_mcp::digest(&bytes),"receipts":receipts,
            "complete":receipts.len()==manifest["heldout"].as_array().unwrap().len(),
            "provider_cost_usd":0,"scope":"Approved native fixture; not a user task or semantic acceptance"})).unwrap()).unwrap();
    }
}
struct Bytes(Vec<u8>);
impl media::Backend for Bytes {
    fn generate(&self, _: &Value) -> anyhow::Result<Vec<u8>> {
        Ok(self.0.clone())
    }
}
fn granted() -> Fixture {
    let f = Fixture::new();
    let db = f.db();
    let mut p = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    p.revision = 2;
    for map in [&mut p.ceilings, &mut p.grants] {
        map.get_mut("Worker")
            .unwrap()
            .tools
            .extend(["image_generate".into(), "audio_generate".into()]);
    }
    governance::save(&db, &f.scope.run.to_string(), &p, 1).unwrap();
    f
}
#[test]
fn generated_binary_publication_is_confined_create_only_and_format_checked() {
    let f = Fixture::new();
    let root = f.authority.workspace(&f.scope).unwrap();
    let dir = workspace::open(&root, true).unwrap();
    let request = json!({"kind":"audio","path":"created/tone.wav","text":"synthetic"});
    let bytes = support::audio::wav(1, 8000, &[1, 0, -1, 0]);
    let value = media::publish(dir.as_ref(), &request, &Bytes(bytes.clone())).unwrap();
    assert_eq!(value["artifact"]["sha256"], gents_local_mcp::digest(&bytes));
    assert_eq!(value["semantic_acceptance"], false);
    assert!(media::publish(dir.as_ref(), &request, &Bytes(bytes.clone())).is_err());
    assert_eq!(
        dir.read_bytes_complete("created/tone.wav", 1024).unwrap(),
        bytes
    );
    for path in ["../outside.wav", "/outside.wav", ".env", "bad.png"] {
        let mut bad = request.clone();
        bad["path"] = json!(path);
        assert!(media::publish(dir.as_ref(), &bad, &Bytes(bytes.clone())).is_err());
    }
    assert!(media::publish(
        dir.as_ref(),
        &json!({"kind":"audio","path":"invalid.wav"}),
        &Bytes(b"not wav".to_vec())
    )
    .is_err());
    assert!(!root.join("invalid.wav").exists());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&root, root.join("escape")).unwrap();
        assert!(media::publish(
            dir.as_ref(),
            &json!({"kind":"audio","path":"escape/clip.wav"}),
            &Bytes(bytes)
        )
        .is_err());
    }
}
#[test]
fn media_has_no_implicit_grants_or_planner_writes_and_frozen_inputs_are_preserved() {
    let f = granted();
    let r = Registry::new();
    let args = json!({"text":"Hello from the local company.","path":"voice.wav"});
    assert!(r
        .call(
            &f.authority,
            &f.actor("Reader"),
            "audio_generate",
            args.clone()
        )
        .is_err());
    let mut planner = f.scope.clone();
    planner.phase = gents_local_mcp::Phase::Planner;
    assert!(r
        .call(&f.authority, &planner, "audio_generate", args.clone())
        .is_err());
    f.db()
        .execute(
            "UPDATE jobs SET intake=?1",
            [json!({"ceo_source_files":[{"path":"voice.wav"}]}).to_string()],
        )
        .unwrap();
    let error = r
        .call(&f.authority, &f.scope, "audio_generate", args.clone())
        .unwrap_err();
    assert!(format!("{error:#}").contains("frozen owner"));
    f.db().execute("UPDATE jobs SET state='draft'", []).unwrap();
    assert!(r
        .call(&f.authority, &f.scope, "audio_generate", args)
        .is_err());
}
#[test]
#[ignore = "opt-in native pinned engines; uses local CPU/GPU and no network"]
fn actual_offline_audio_and_image_generators_publish_hash_bound_files() {
    std::env::set_current_dir(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap(),
    )
    .unwrap();
    let f = granted();
    let r = Registry::new();
    let image=r.call(&f.authority,&f.scope,"image_generate",json!({"prompt":"A friendly ivory owl with a black bow tie, warm gray studio background.","path":"owl.png","width":256,"height":256,"seed":42})).unwrap();
    let audio=r.call(&f.authority,&f.scope,"audio_generate",json!({"text":"Welcome to your local company. All generation stays on this computer.","path":"welcome.wav"})).unwrap();
    for reply in [&image, &audio] {
        assert_eq!(reply["value"]["provider_cost_usd"], 0);
        assert_eq!(reply["value"]["semantic_acceptance"], false);
    }
    let root = f.authority.workspace(&f.scope).unwrap();
    let retained = std::env::current_dir()
        .unwrap()
        .join("data/mcp-basement-verification");
    std::fs::create_dir_all(&retained).unwrap();
    std::fs::copy(
        root.join("owl.png"),
        retained.join("mcp-local-generated-owl.png"),
    )
    .unwrap();
    std::fs::copy(
        root.join("welcome.wav"),
        retained.join("mcp-local-generated-welcome.wav"),
    )
    .unwrap();
    std::fs::write(retained.join("mcp-media-native-receipts.json"),serde_json::to_vec_pretty(&json!({"image":image,"audio":audio,"provider_cost_usd":0,"scope":"Actual local native tool generation with fixture owner approval and exact tool grants. No task completion or semantic quality acceptance claimed."})).unwrap()).unwrap();
}
