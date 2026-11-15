//! Pure type checks for future receipt-backed component edges. No receipt lookup,
//! source activation, authority, coercion, compilation or candidate execution.
use super::{functions, types::WireType};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

pub(super) fn projected<'a>(mut ty: &'a WireType, pointer: &str) -> Result<&'a WireType> {
    ensure!(
        pointer.len() <= 160 && !pointer.chars().any(char::is_control),
        "component projection pointer exceeds bounds"
    );
    if pointer.is_empty() {
        return Ok(ty);
    }
    let parts = pointer
        .strip_prefix('/')
        .context("component projection must be a JSON pointer")?
        .split('/')
        .collect::<Vec<_>>();
    ensure!(
        parts.len() <= 6,
        "component projection nesting exceeds bound"
    );
    for part in parts {
        ty = match (ty, part) {
            (WireType::Option(inner), "some") => inner,
            (WireType::Result(ok, _), "ok") => ok,
            (WireType::Result(_, err), "err") => err,
            (WireType::Tuple(types), index) => &types[canonical_index(index, types.len())?],
            (WireType::Vector(inner) | WireType::Slice(inner), index) => {
                canonical_index(index, 256)?;
                inner
            }
            _ => anyhow::bail!("component projection does not select a declared data value"),
        };
    }
    Ok(ty)
}
fn canonical_index(text: &str, bound: usize) -> Result<usize> {
    let index = text
        .parse::<usize>()
        .context("component array index required")?;
    ensure!(
        index < bound && index.to_string() == text,
        "invalid component array index"
    );
    Ok(index)
}

/// Describes one statically compatible edge between inspected source interfaces.
/// Tags/indices may be absent at runtime; a future resolver must fail closed then.
/// Source/API custody, exact approved graph and canonical receipt lineage remain
/// obligations of the caller. Identical wire schemas allow Vec/slice and String/
/// str adapters, but never numeric coercion, fallback tags or invented values.
pub fn describe(
    producer_source: &str,
    producer_function: &str,
    output_pointer: &str,
    consumer_source: &str,
    consumer_function: &str,
    argument_index: usize,
) -> Result<Value> {
    let producers = functions(producer_source)?;
    let consumers = functions(consumer_source)?;
    let producer = producers
        .iter()
        .find(|f| f.name == producer_function)
        .context("unknown producer function")?;
    let consumer = consumers
        .iter()
        .find(|f| f.name == consumer_function)
        .context("unknown consumer function")?;
    let input = consumer
        .inputs
        .get(argument_index)
        .context("consumer argument index out of range")?;
    let output = projected(&producer.output, output_pointer)?;
    ensure!(
        output.schema() == input.schema(),
        "component edge wire types differ"
    );
    let producer_api = super::describe(producer_source)?;
    let consumer_api = super::describe(consumer_source)?;
    Ok(
        json!({"producer_source_sha256":producer_api["source_sha256"],
        "producer_api_sha256":producer_api["api_sha256"],"producer_function":producer_function,
        "output_pointer":output_pointer,"consumer_source_sha256":consumer_api["source_sha256"],
        "consumer_api_sha256":consumer_api["api_sha256"],"consumer_function":consumer_function,
        "argument_index":argument_index,"wire_schema":input.schema(),
        "scope":"Static interface compatibility only; runtime presence, owner approval, source custody, receipt lineage, actor policy and bounded value validation still required. No execution or authority."}),
    )
}

/// Validates a value produced by the bounded native ABI without executing code.
pub fn validate_output(source: &str, function: &str, value: &Value) -> Result<()> {
    ensure!(
        serde_json::to_vec(value)?.len() <= 8192,
        "component output exceeds bound"
    );
    let interfaces = functions(source)?;
    let interface = interfaces
        .iter()
        .find(|f| f.name == function)
        .context("unknown component function")?;
    interface.output.literal(value, 0).map(|_| ())
}

/// Selects an actually present typed value. A missing Option/Result branch or
/// vector element is an error, never null, a default, or a model-supplied value.
/// Caller must obtain `value` from verified current-run native evidence; this
/// pure helper does not establish provenance or authorize consuming the value.
pub fn project_output<'a>(
    source: &str,
    function: &str,
    pointer: &str,
    value: &'a Value,
) -> Result<&'a Value> {
    validate_output(source, function, value)?;
    let interfaces = functions(source)?;
    let interface = interfaces
        .iter()
        .find(|f| f.name == function)
        .context("unknown component function")?;
    projected(&interface.output, pointer)?;
    value
        .pointer(pointer)
        .context("component projection is absent in the actual native result")
}

#[cfg(test)]
mod tests {
    use super::*;
    const PRODUCER: &str = "pub fn make()->Option<Vec<u64>>{Some(Vec::new())}";
    const CONSUMER: &str = "pub fn use_values(values:&[u64])->usize{values.len()}";
    #[test]
    fn inspected_edges_bind_exact_interfaces_without_running_candidates() {
        let edge = describe(PRODUCER, "make", "/some", CONSUMER, "use_values", 0).unwrap();
        assert_eq!(edge["wire_schema"]["type"], "array");
        assert_eq!(edge["wire_schema"]["items"]["x-rust-type"], "u64");
        assert_eq!(
            edge["producer_source_sha256"],
            crate::digest(PRODUCER.as_bytes())
        );
        let trap = "pub fn make()->Vec<u64>{loop{}}";
        assert!(describe(trap, "make", "", CONSUMER, "use_values", 0).is_ok());
        assert!(describe(PRODUCER, "make", "", CONSUMER, "use_values", 0).is_err());
        for consumer in [
            "pub fn use_values(v:&[i64])->usize{v.len()}",
            "pub fn use_values(v:&[u32])->usize{v.len()}",
        ] {
            assert!(describe(PRODUCER, "make", "/some", consumer, "use_values", 0).is_err());
        }
        assert!(describe(PRODUCER, "make", "/some", CONSUMER, "use_values", 1).is_err());
        assert!(describe(PRODUCER, "missing", "/some", CONSUMER, "use_values", 0).is_err());
    }
    #[test]
    fn projections_select_only_declared_tags_and_bounded_canonical_indices() {
        let consumer = "pub fn take(v:u64)->u64{v}";
        for pointer in ["/some/0", "/some/255"] {
            assert!(describe(PRODUCER, "make", pointer, consumer, "take", 0).is_ok());
        }
        for pointer in [
            "some",
            "/none",
            "/some/-",
            "/some/00",
            "/some/+0",
            "/some/-1",
            "/some/256",
            "/some/0/0",
            "/some/~0",
            "/some/0\n",
        ] {
            assert!(
                describe(PRODUCER, "make", pointer, consumer, "take", 0).is_err(),
                "{pointer:?}"
            );
        }
        let tuple = "pub fn make()->Result<(bool,u64),String>{Ok((true,1))}";
        assert!(describe(tuple, "make", "/ok/1", consumer, "take", 0).is_ok());
        assert!(describe(tuple, "make", "/ok/2", consumer, "take", 0).is_err());
        assert!(describe(tuple, "make", "/ok/0", consumer, "take", 0).is_err());
    }
    #[test]
    fn runtime_outputs_retain_tag_range_size_and_canonical_integer_checks() {
        assert!(validate_output(
            PRODUCER,
            "make",
            &json!({"some":["0","18446744073709551615"]})
        )
        .is_ok());
        assert!(validate_output(PRODUCER, "make", &json!({"none":true})).is_ok());
        for value in [
            json!({"some":["01"]}),
            json!({"some":[1]}),
            json!({"some":["18446744073709551616"]}),
            json!({"some":[],"none":true}),
            json!({"none":false}),
            json!({"some":vec!["0";257]}),
        ] {
            assert!(validate_output(PRODUCER, "make", &value).is_err());
        }
        let text = "pub fn make()->String{String::new()}";
        assert!(validate_output(text, "make", &json!("x".repeat(4097))).is_err());
        assert!(validate_output(text, "make", &json!("x".repeat(8193))).is_err());
    }
    #[test]
    fn absent_tags_and_elements_never_become_defaults_or_coercions() {
        let value = json!({"some":["7"]});
        assert_eq!(
            project_output(PRODUCER, "make", "/some/0", &value).unwrap(),
            &json!("7")
        );
        assert!(project_output(PRODUCER, "make", "/some/1", &value).is_err());
        assert!(project_output(PRODUCER, "make", "/some", &json!({"none":true})).is_err());
        assert!(project_output(PRODUCER, "make", "/none", &json!({"none":true})).is_err());
        assert!(project_output(PRODUCER, "make", "/some/0", &json!({"some":[7]})).is_err());
        assert!(project_output(PRODUCER, "make", "/some/00", &value).is_err());
        assert_eq!(
            project_output(PRODUCER, "make", "", &value).unwrap(),
            &value
        );
        let result = "pub fn make()->Result<u64,String>{Ok(7)}";
        assert!(project_output(result, "make", "/ok", &json!({"err":"unavailable"})).is_err());
    }
}
