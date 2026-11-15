mod arithmetic;
mod audio_inspect;
mod files;
mod host_resources;
mod media_generate;
mod memory;
mod owned_rust_call;
mod rust_verify;
mod skills;
mod table;
mod table_join;
mod table_read;
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
        Box::new(owned_rust_call::OwnedRustCall),
    ]
}

pub use table_read::validate_text as validate_table_text;
