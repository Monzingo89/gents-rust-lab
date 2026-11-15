use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use syn::{GenericArgument, PathArguments, Type};

pub(super) enum WireType {
    Unit,
    Bool,
    Integer(String),
    Text,
    Tuple(Vec<Self>),
    Vector(Box<Self>),
    Option(Box<Self>),
    Result(Box<Self>, Box<Self>),
    Slice(Box<Self>),
    Str,
}
impl WireType {
    pub fn parse(ty: &Type, input: bool, depth: usize) -> Result<Self> {
        ensure!(depth <= 5, "component type nesting exceeds bound");
        Ok(match ty {
            Type::Tuple(tuple) if tuple.elems.len() <= 6 => {
                if tuple.elems.is_empty() {
                    Self::Unit
                } else {
                    Self::Tuple(
                        tuple
                            .elems
                            .iter()
                            .map(|t| Self::parse(t, false, depth + 1))
                            .collect::<Result<_>>()?,
                    )
                }
            }
            Type::Reference(reference)
                if input && reference.mutability.is_none() && reference.lifetime.is_none() =>
            {
                match reference.elem.as_ref() {
                    Type::Slice(slice) => {
                        Self::Slice(Box::new(Self::parse(&slice.elem, false, depth + 1)?))
                    }
                    Type::Path(p) if p.path.is_ident("str") => Self::Str,
                    _ => anyhow::bail!("component accepts only immutable slice/str references"),
                }
            }
            Type::Path(path) if path.qself.is_none() && path.path.leading_colon.is_none() => {
                let segments = &path.path.segments;
                let last = segments.last().context("missing component type")?;
                let name = last.ident.to_string();
                let prefix = segments
                    .iter()
                    .take(segments.len() - 1)
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::");
                let expected = match name.as_str() {
                    "Vec" => "vec",
                    "String" => "string",
                    "Option" => "option",
                    "Result" => "result",
                    _ => "primitive",
                };
                ensure!(
                    prefix.is_empty()
                        || prefix == format!("std::{expected}")
                        || prefix == format!("core::{expected}"),
                    "unsupported component type path"
                );
                let arguments = match &last.arguments {
                    PathArguments::None => Vec::new(),
                    PathArguments::AngleBracketed(a) => a
                        .args
                        .iter()
                        .map(|a| match a {
                            GenericArgument::Type(t) => Self::parse(t, false, depth + 1),
                            _ => anyhow::bail!("unsupported component type argument"),
                        })
                        .collect::<Result<Vec<_>>>()?,
                    _ => anyhow::bail!("unsupported component callable type"),
                };
                let mut arguments = arguments.into_iter();
                match name.as_str() {
                    "Vec" => Self::Vector(Box::new(arguments.next().context("Vec needs a type")?)),
                    "Option" => {
                        Self::Option(Box::new(arguments.next().context("Option needs a type")?))
                    }
                    "Result" => Self::Result(
                        Box::new(arguments.next().context("Result needs types")?),
                        Box::new(arguments.next().context("Result needs two types")?),
                    ),
                    "bool" if arguments.len() == 0 => Self::Bool,
                    "String" if arguments.len() == 0 => Self::Text,
                    "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize"
                    | "isize"
                        if arguments.len() == 0 =>
                    {
                        Self::Integer(name)
                    }
                    _ => anyhow::bail!("unsupported component type"),
                }
                .checked(arguments.len())?
            }
            _ => anyhow::bail!("unsupported component type"),
        })
    }
    fn checked(self, extra: usize) -> Result<Self> {
        ensure!(extra == 0, "extra component type arguments");
        Ok(self)
    }
    pub fn schema(&self) -> Value {
        match self {
            Self::Unit => json!({"type":"null"}),
            Self::Bool => json!({"type":"boolean"}),
            Self::Integer(name) => {
                json!({"type":"string","pattern":if name.starts_with('u') {"^(0|[1-9][0-9]*)$"} else {"^(0|-?[1-9][0-9]*)$"},"maxLength":20,"x-rust-type":name,"x-rust-bits":integer_bits(name)})
            }
            Self::Text | Self::Str => json!({"type":"string","maxLength":4096}),
            Self::Vector(t) | Self::Slice(t) => {
                json!({"type":"array","items":t.schema(),"maxItems":256})
            }
            Self::Tuple(types) => {
                json!({"type":"array","prefixItems":types.iter().map(Self::schema).collect::<Vec<_>>(),"minItems":types.len(),"maxItems":types.len(),"items":false})
            }
            Self::Option(t) => {
                json!({"oneOf":[object("none",json!({"const":true})),object("some",t.schema())]})
            }
            Self::Result(ok, err) => {
                json!({"oneOf":[object("ok",ok.schema()),object("err",err.schema())]})
            }
        }
    }
    pub fn literal(&self, value: &Value, depth: usize) -> Result<String> {
        ensure!(depth <= 6, "component argument nesting exceeds bound");
        Ok(match self {
            Self::Unit => {
                ensure!(value.is_null(), "expected unit/null");
                "()".into()
            }
            Self::Bool => value.as_bool().context("expected boolean")?.to_string(),
            Self::Integer(name) => {
                let text = value
                    .as_str()
                    .context("integers require canonical decimal strings")?;
                let valid = if name.starts_with('u') {
                    text.parse::<u64>().is_ok_and(|n| {
                        n.to_string() == text
                            && (integer_bits(name) == 64 || n < 1_u64 << integer_bits(name))
                    })
                } else {
                    text.parse::<i64>().is_ok_and(|n| {
                        n.to_string() == text
                            && (integer_bits(name) == 64 || {
                                let bits = integer_bits(name);
                                n >= -(1_i64 << (bits - 1)) && n < 1_i64 << (bits - 1)
                            })
                    })
                };
                ensure!(valid, "invalid or out-of-range canonical integer");
                format!("{text}{name}")
            }
            Self::Text | Self::Str => {
                let text = value.as_str().context("expected string")?;
                ensure!(text.len() <= 4096, "component text exceeds bound");
                let quoted = format!("{text:?}");
                if matches!(self, Self::Str) {
                    quoted
                } else {
                    format!("String::from({quoted})")
                }
            }
            Self::Tuple(types) => {
                let values = value.as_array().context("expected tuple array")?;
                ensure!(values.len() == types.len(), "tuple arity mismatch");
                format!(
                    "({},)",
                    types
                        .iter()
                        .zip(values)
                        .map(|(t, v)| t.literal(v, depth + 1))
                        .collect::<Result<Vec<_>>>()?
                        .join(",")
                )
            }
            Self::Vector(t) | Self::Slice(t) => {
                let values = value.as_array().context("expected array")?;
                ensure!(values.len() <= 256, "component array exceeds bound");
                let values = values
                    .iter()
                    .map(|v| t.literal(v, depth + 1))
                    .collect::<Result<Vec<_>>>()?
                    .join(",");
                if matches!(self, Self::Slice(_)) {
                    format!("&[{values}]")
                } else {
                    format!("Vec::from([{values}])")
                }
            }
            Self::Option(t) => {
                let fields = value.as_object().context("expected tagged Option")?;
                ensure!(fields.len() == 1, "Option needs exactly one tag");
                if value.get("none") == Some(&json!(true)) {
                    "None".into()
                } else {
                    format!(
                        "Some({})",
                        t.literal(
                            value.get("some").context("expected some/none tag")?,
                            depth + 1
                        )?
                    )
                }
            }
            Self::Result(ok, err) => {
                let fields = value.as_object().context("expected tagged Result")?;
                ensure!(fields.len() == 1, "Result needs exactly one tag");
                if let Some(v) = value.get("ok") {
                    format!("Ok({})", ok.literal(v, depth + 1)?)
                } else {
                    format!(
                        "Err({})",
                        err.literal(value.get("err").context("expected ok/err tag")?, depth + 1)?
                    )
                }
            }
        })
    }
    pub fn encode(&self, value: &str, depth: usize) -> String {
        let item = format!("item{depth}");
        match self {
            Self::Unit => "wire.text(\"null\")".into(),
            Self::Bool => format!("wire.text(if {value} {{\"true\"}} else {{\"false\"}})"),
            Self::Integer(_) => format!("wire.string(&{value}.to_string())"),
            Self::Text | Self::Str => format!("wire.string(&{value})"),
            Self::Tuple(types) => {
                let parts=types.iter().enumerate().map(|(i,t)|format!("{}{};",if i>0{"wire.text(\",\");"}else{""},t.encode(&format!("{value}.{i}"),depth+1))).collect::<String>();
                format!("wire.text(\"[\"); {parts} wire.text(\"]\")")
            }
            Self::Vector(t) | Self::Slice(t) => format!("wire.text(\"[\"); if {value}.len()>256 {{std::process::exit(67);}} for (index{depth}, {item}) in {value}.into_iter().enumerate() {{let _=&{item};if index{depth}>0{{wire.text(\",\");}} {};}} wire.text(\"]\")",t.encode(&item,depth+1)),
            Self::Option(t) => format!("match {value} {{None=>wire.text(\"{{\\\"none\\\":true}}\"),Some({item})=>{{let _=&{item};wire.text(\"{{\\\"some\\\":\"); {};wire.text(\"}}\");}}}}",t.encode(&item,depth+1)),
            Self::Result(ok,err) => format!("match {value} {{Ok({item})=>{{let _=&{item};wire.text(\"{{\\\"ok\\\":\"); {};wire.text(\"}}\");}},Err({item})=>{{let _=&{item};wire.text(\"{{\\\"err\\\":\"); {};wire.text(\"}}\");}}}}",ok.encode(&item,depth+1),err.encode(&item,depth+1)),
        }
    }
}
fn integer_bits(name: &str) -> u32 {
    match name {
        "usize" | "isize" => usize::BITS,
        _ => name[1..].parse().expect("validated integer type"),
    }
}
fn object(tag: &str, value: Value) -> Value {
    json!({"type":"object","additionalProperties":false,"required":[tag],"properties":{tag:value}})
}
