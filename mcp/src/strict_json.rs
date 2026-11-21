//! Recursive duplicate-key rejection, shared by bounded JSON consumers.
use serde::Deserialize;
use serde_json::{json, Value};
// A metadata observation must not choose the last of duplicate decoded keys.
// This is recursive: a duplicate dependency or partition property is ambiguous
// even when the surrounding source JSON is otherwise syntactically valid.
pub(crate) struct UniqueValue(pub(crate) Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> std::result::Result<Self, D::Error> {
        struct Unique;
        impl<'de> serde::de::Visitor<'de> for Unique {
            type Value = UniqueValue;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("JSON with unique decoded object keys")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut entries: M,
            ) -> std::result::Result<Self::Value, M::Error> {
                let mut values = serde_json::Map::new();
                while let Some((key, value)) = entries.next_entry::<String, UniqueValue>()? {
                    if values.insert(key, value.0).is_some() {
                        return Err(serde::de::Error::custom("duplicate JSON object key"));
                    }
                }
                Ok(UniqueValue(Value::Object(values)))
            }
            fn visit_seq<S: serde::de::SeqAccess<'de>>(
                self,
                mut entries: S,
            ) -> std::result::Result<Self::Value, S::Error> {
                let mut values = vec![];
                while let Some(value) = entries.next_element::<UniqueValue>()? {
                    values.push(value.0);
                }
                Ok(UniqueValue(Value::Array(values)))
            }
            fn visit_bool<E: serde::de::Error>(
                self,
                v: bool,
            ) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(json!(v)))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(json!(v)))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(json!(v)))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Self::Value, E> {
                serde_json::Number::from_f64(v)
                    .map(|v| UniqueValue(Value::Number(v)))
                    .ok_or_else(|| E::custom("non-finite JSON value"))
            }
            fn visit_str<E: serde::de::Error>(
                self,
                v: &str,
            ) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(json!(v)))
            }
            fn visit_string<E: serde::de::Error>(
                self,
                v: String,
            ) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(Value::String(v)))
            }
            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }
        }
        decoder.deserialize_any(Unique)
    }
}

pub(crate) fn parse(text: &str) -> anyhow::Result<Value> {
    Ok(serde_json::from_str::<UniqueValue>(text)?.0)
}
