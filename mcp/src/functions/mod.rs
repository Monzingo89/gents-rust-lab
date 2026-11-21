mod angular_check;
mod arithmetic;
mod audio_inspect;
mod dotnet_check;
mod files;
mod host_resources;
mod media_generate;
mod memory;
mod owned_rust_call;
mod python_check;
mod rust_verify;
mod skills;
mod table;
mod table_join;
mod table_read;
mod tasks;
pub fn builtins() -> Vec<Box<dyn crate::Tool>> {
    vec![
        Box::new(arithmetic::IntegerArithmetic),
        Box::new(audio_inspect::AudioInspect),
        Box::new(media_generate::ImageGenerate),
        Box::new(media_generate::AudioGenerate),
        Box::new(table::TableAggregate),
        Box::new(table_join::TableJoin),
        Box::new(table_read::TableReadJson),
        Box::new(memory::Append),
        Box::new(memory::Search),
        Box::new(skills::List),
        Box::new(skills::Read),
        Box::new(files::Files),
        Box::new(host_resources::HostResources),
        Box::new(rust_verify::RustVerify),
        Box::new(python_check::PythonCheck),
        Box::new(angular_check::AngularCheck),
        Box::new(dotnet_check::DotnetCheck(false)),
        Box::new(dotnet_check::DotnetCheck(true)),
        Box::new(owned_rust_call::OwnedRustCall),
        Box::new(tasks::TaskTool("destructure_task")),
        Box::new(tasks::TaskTool("list_capabilities")),
        Box::new(tasks::TaskTool("discover_models")),
        Box::new(tasks::TaskTool("execute_task")),
        Box::new(tasks::TaskTool("get_task_status")),
        Box::new(tasks::TaskTool("cancel_task")),
        Box::new(tasks::TaskTool("ingest_sources")),
        Box::new(tasks::TaskTool("search_knowledge")),
        Box::new(tasks::TaskTool("inspect_cache")),
        Box::new(tasks::TaskTool("invalidate_cache")),
    ]
}

pub use table_read::validate_text as validate_table_text;
/// The async/task service has its own typed intake/catalog. Legacy company
/// prompts keep their established packet budget; native tools/list still shows
/// these tools when the current actor has an explicit grant.
pub fn is_task_capability(name: &str) -> bool {
    matches!(
        name,
        "destructure_task"
            | "list_capabilities"
            | "discover_models"
            | "execute_task"
            | "get_task_status"
            | "cancel_task"
            | "ingest_sources"
            | "search_knowledge"
            | "inspect_cache"
            | "invalidate_cache"
    )
}
