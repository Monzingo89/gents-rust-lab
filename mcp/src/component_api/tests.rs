use super::*;
use serde_json::json;

#[test]
fn interface_identity_is_not_source_identity_or_authority() {
    let a = describe("pub fn protocol_value(x:u64)->u64{x}").unwrap();
    let b = describe("pub fn protocol_value(x:u64)->u64{x+1}").unwrap();
    assert_eq!(a["api_sha256"], b["api_sha256"]);
    assert_ne!(a["source_sha256"], b["source_sha256"]);
    assert_eq!(a["execution_authorized"], false);
    let c = describe("pub fn protocol_value(x:u32)->u64{x as u64}").unwrap();
    assert_ne!(a["api_sha256"], c["api_sha256"]);
    assert_eq!(
        a["interface"]["functions"][0]["arguments"]["prefixItems"][0]["type"],
        "string"
    );
}
#[test]
fn unsupported_or_effectful_apis_and_wire_injection_fail_closed() {
    for source in [
        "pub fn f<T>(x:T)->T{x}",
        "pub fn f(x:&mut [u8]){}",
        "pub fn f()->std::fs::File{loop{}}",
        "pub fn f(x:f64)->f64{x}",
        "pub fn f(x:Option<u8,u16>){}",
    ] {
        assert!(describe(source).is_err(), "{source}");
    }
    let source = "pub fn protocol_value(x:u8)->u8{x}";
    for value in [
        json!(1),
        json!("01"),
        json!("-0"),
        json!("256"),
        json!("1);std::process::exit(0);("),
    ] {
        assert!(driver(source, "protocol_value", &json!([value])).is_err());
    }
    assert!(driver(source, "protocol_value();", &json!(["1"])).is_err());
    assert!(driver(source, "protocol_value", &json!(["1", "2"])).is_err());
}
#[test]
fn machine_integer_width_is_part_of_the_described_interface() {
    let d = describe("pub fn protocol_value(x:usize)->usize{x}").unwrap();
    assert_eq!(
        d["interface"]["functions"][0]["result"]["x-rust-bits"],
        usize::BITS
    );
    assert!(driver(
        "pub fn protocol_value(x:usize)->usize{x}",
        "protocol_value",
        &json!([usize::MAX.to_string()])
    )
    .is_ok());
}
#[cfg(target_os = "macos")]
#[test]
fn unit_values_empty_containers_and_slice_arguments_compile_without_special_cases() {
    for (source, args, expected) in [
        (
            "pub fn protocol_value(x:Option<()>)->Option<()>{x}",
            json!([{"some":null}]),
            json!({"some":null}),
        ),
        (
            "pub fn protocol_value(x:Result<(),()>)->Result<(),()>{x}",
            json!([{"err":null}]),
            json!({"err":null}),
        ),
        (
            "pub fn protocol_value(x:&[(u64,u8)])->Vec<(u64,u8)>{x.to_vec()}",
            json!([[]]),
            json!([]),
        ),
        (
            "pub fn protocol_value(x:&str)->String{x.to_string()}",
            json!(["😀\n\\\""]),
            json!("😀\n\\\""),
        ),
        ("pub fn protocol_value(){}", json!([]), json!(null)),
    ] {
        let d = describe(source).unwrap();
        let r = invoke(
            source,
            d["source_sha256"].as_str().unwrap(),
            d["api_sha256"].as_str().unwrap(),
            "protocol_value",
            &args,
        )
        .unwrap();
        assert_eq!(r["passed"], true, "{r}");
        assert_eq!(r["output"], expected);
    }
}
#[test]
fn tags_preserve_nested_none_and_decimal_integer_extremes() {
    let source = "pub fn protocol_value(x:Option<Option<i64>>)->Option<Option<i64>>{x}";
    assert!(
        driver(source, "protocol_value", &json!([{"some":{"none":true}}]))
            .unwrap()
            .contains("Some(None)")
    );
    assert!(driver(source, "protocol_value", &json!([null])).is_err());
    assert!(driver(
        source,
        "protocol_value",
        &json!([{"none":true,"some":null}])
    )
    .is_err());
    assert!(driver(
        source,
        "protocol_value",
        &json!([{"some":{"some":"-9223372036854775808"}}])
    )
    .is_ok());
    assert!(driver(
        source,
        "protocol_value",
        &json!([{"some":{"some":"-9223372036854775809"}}])
    )
    .is_err());
}
#[cfg(target_os = "macos")]
#[test]
fn real_isolated_codec_roundtrips_losslessly_and_checks_both_pins() {
    let source="pub fn protocol_value(x:(u64,Option<Option<i64>>,Result<String,bool>,Vec<(u8,String)>))->(u64,Option<Option<i64>>,Result<String,bool>,Vec<(u8,String)>){x}";
    let description = describe(source).unwrap();
    let args = json!([["18446744073709551615",{"some":{"some":"-9223372036854775808"}},{"ok":"quote\"\\\n\u{0008}😀"},[["255","Ω"]]]]);
    let invoke = |s: &str, a: &str| super::invoke(source, s, a, "protocol_value", &args);
    assert!(invoke(&"0".repeat(64), description["api_sha256"].as_str().unwrap()).is_err());
    assert!(invoke(
        description["source_sha256"].as_str().unwrap(),
        &"0".repeat(64)
    )
    .is_err());
    let result = invoke(
        description["source_sha256"].as_str().unwrap(),
        description["api_sha256"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(result["passed"], true, "{result}");
    assert_eq!(result["output"], args[0]);
    assert_eq!(result["provider_cost_usd"], 0);
}
#[cfg(target_os = "macos")]
#[test]
fn nontermination_and_unrepresentable_output_never_pass() {
    for source in [
        "pub fn protocol_value()->u8{loop{}}",
        "pub fn protocol_value()->Vec<u8>{let mut v=Vec::new();v.resize(257,0);v}",
    ] {
        let d = describe(source).unwrap();
        let result = invoke(
            source,
            d["source_sha256"].as_str().unwrap(),
            d["api_sha256"].as_str().unwrap(),
            "protocol_value",
            &json!([]),
        )
        .unwrap();
        assert_eq!(result["passed"], false, "{result}");
        assert!(result["output"].is_null());
    }
}

#[test]
fn partial_input_validation_checks_fixed_literals_without_executing_or_filling_slots() {
    let source = "pub fn protocol_template(x:u64,values:&[u64])->u64{let _=values;loop{}}";
    assert!(
        super::validate_template(source, "protocol_template", &json!([null, ["7"]]), &[0]).is_ok()
    );
    assert!(super::validate_template(source, "protocol_template", &json!(["7", []]), &[1]).is_ok());
    for (args, slots) in [
        (json!(["01", []]), vec![1]),
        (json!(["7", []]), vec![0]),
        (json!([null, []]), vec![0, 0]),
        (json!([null, []]), vec![2]),
        (json!([null, []]), vec![]),
    ] {
        assert!(super::validate_template(source, "protocol_template", &args, &slots).is_err());
    }
}
