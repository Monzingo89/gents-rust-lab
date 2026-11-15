//! Bounded data ABI for owned pure Rust libraries. Descriptions grant no authority.
//! Transport, source custody, approval and invocation remain separate adapters.
pub mod connections;
pub mod metadata;
mod types;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use syn::{FnArg, Item, ReturnType, Visibility};
use types::WireType;

struct Function {
    name: String,
    inputs: Vec<WireType>,
    output: WireType,
}
fn functions(source: &str) -> Result<Vec<Function>> {
    ensure!(source.len() <= 8192, "component source exceeds bound");
    ensure!(
        crate::rust_verification::source_policy_observation(source)["passed"] == true,
        "component source must pass the native pure Rust dialect"
    );
    let parsed = syn::parse_file(source).context("component source syntax")?;
    let mut functions = Vec::new();
    for item in parsed.items {
        let Item::Fn(function) = item else {
            unreachable!()
        };
        if !matches!(function.vis, Visibility::Public(_)) {
            continue;
        }
        let sig = &function.sig;
        ensure!(
            sig.generics.params.is_empty()
                && sig.generics.where_clause.is_none()
                && sig.variadic.is_none()
                && sig.inputs.len() <= 8,
            "component API excludes generic/variadic functions and more than eight arguments"
        );
        let mut inputs = Vec::new();
        for argument in &sig.inputs {
            let FnArg::Typed(argument) = argument else {
                anyhow::bail!("component methods unsupported")
            };
            inputs.push(WireType::parse(&argument.ty, true, 0)?);
        }
        let output = match &sig.output {
            ReturnType::Default => WireType::Unit,
            ReturnType::Type(_, ty) => WireType::parse(ty, false, 0)?,
        };
        functions.push(Function {
            name: sig.ident.to_string(),
            inputs,
            output,
        });
        ensure!(
            functions.len() <= 16,
            "component exposes more than sixteen functions"
        );
    }
    ensure!(
        !functions.is_empty(),
        "component exposes no supported public functions"
    );
    Ok(functions)
}

/// Exact source and interface identities; never an activation or permission record.
pub fn describe(source: &str) -> Result<Value> {
    let functions = functions(source)?;
    let interface = json!({"abi":"gents-pure-rust-json-v1", "functions":functions.iter().map(|f| {
        json!({"name":f.name,"arguments":{"type":"array","minItems":f.inputs.len(),"maxItems":f.inputs.len(),
            "prefixItems":f.inputs.iter().map(WireType::schema).collect::<Vec<_>>(),"items":false},"result":f.output.schema()})
    }).collect::<Vec<_>>(),"integer_encoding":"canonical decimal strings, range checked; no floating point; machine-size widths bind this host API identity",
        "scope":"Source parser and supported data ABI only. No compilation, correctness, qualification, invocation, authorization, promotion or industry assurance."});
    Ok(
        json!({"source_sha256":crate::digest(source.as_bytes()),"api_sha256":crate::digest(&serde_json::to_vec(&interface)?),
        "interface":interface,"execution_authorized":false,"promotion_authorized":false}),
    )
}

/// Validate bounded argument marshalling without compiling or invoking source.
/// The caller must separately establish current source custody and authority.
pub fn validate_arguments(source: &str, name: &str, args: &Value) -> Result<()> {
    // Build bounded codec expressions only; never compile or invoke a candidate.
    driver(source, name, args).map(|_| ())
}

/// Validate owner-held literals and explicitly empty slots without inventing a
/// value for a missing argument. Source parsing/codec checks only, no execution.
pub fn validate_template(source: &str, name: &str, args: &Value, slots: &[usize]) -> Result<()> {
    if slots.is_empty() {
        return validate_arguments(source, name, args);
    }
    let functions = functions(source)?;
    let function = functions
        .iter()
        .find(|f| f.name == name)
        .context("unknown component function")?;
    let args = args
        .as_array()
        .context("template arguments must be positional")?;
    ensure!(
        args.len() == function.inputs.len() && serde_json::to_vec(args)?.len() <= 8192,
        "template argument count or size mismatch"
    );
    let selected = slots
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    ensure!(
        selected.len() == slots.len()
            && selected.len() <= 8
            && selected.iter().all(|i| *i < args.len()),
        "invalid or duplicate template argument slots"
    );
    for (index, (ty, value)) in function.inputs.iter().zip(args).enumerate() {
        if selected.contains(&index) {
            ensure!(
                crate::artifact_bindings::is_placeholder(value),
                "template slot must be empty"
            );
        } else {
            ty.literal(value, 0)?;
        }
    }
    Ok(())
}

/// Private driver generation shared by validation and isolated execution.
pub(crate) fn driver(source: &str, name: &str, args: &Value) -> Result<String> {
    let functions = functions(source)?;
    let function = functions
        .iter()
        .find(|f| f.name == name)
        .context("unknown component function")?;
    let args = args
        .as_array()
        .context("component arguments must be positional array")?;
    ensure!(
        args.len() == function.inputs.len(),
        "component argument count mismatch"
    );
    ensure!(
        serde_json::to_vec(args)?.len() <= 8192,
        "component arguments exceed bound"
    );
    let expressions = function
        .inputs
        .iter()
        .zip(args)
        .map(|(ty, value)| ty.literal(value, 0))
        .collect::<Result<Vec<_>>>()?;
    let encode = function.output.encode("value", 0);
    Ok(format!("{}\nfn main() {{ let value = candidate::{}({}); let _ = &value; let mut wire = Wire(String::new()); {}; std::io::Write::write_all(&mut std::io::stdout().lock(),wire.0.as_bytes()).unwrap(); }}",
        include_str!("wire.inc"), function.name, expressions.join(","), encode))
}

/// Isolated execution capability, not a governance decision. Native adapters
/// must bind exact source/API hashes to an owner-approved run and current actor.
pub fn invoke(
    source: &str,
    source_sha256: &str,
    api_sha256: &str,
    name: &str,
    args: &Value,
) -> Result<Value> {
    let description = describe(source)?;
    ensure!(
        description["source_sha256"] == source_sha256 && description["api_sha256"] == api_sha256,
        "component source/API pin mismatch"
    );
    let driver = driver(source, name, args)?;
    let mut result = crate::rust_verification::sandbox::invoke(source, &driver)?;
    if result["passed"] == true {
        connections::validate_output(
            source,
            name,
            result
                .get("output")
                .context("successful component lacks output")?,
        )?;
    }
    result["source_sha256"] = description["source_sha256"].clone();
    result["api_sha256"] = description["api_sha256"].clone();
    result["function"] = json!(name);
    result["provider_cost_usd"] = json!(0);
    result["promotion_authorized"] = json!(false);
    Ok(result)
}

#[cfg(test)]
mod tests;
