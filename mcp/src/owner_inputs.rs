//! Frozen owner inputs are data sources, never agent write destinations.
use serde_json::Value;
fn paths(intake: &Value) -> Vec<&str> {
    let mut paths = Vec::new();
    intake["required_owned_rust_calls"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|call| call["source_path"].as_str())
        .for_each(|path| paths.push(path));
    intake["required_rust_checks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|check| check["tests_path"].as_str())
        .for_each(|path| paths.push(path));
    intake["python_check_contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|check| check["tests_path"].as_str())
        .for_each(|path| paths.push(path));
    intake["python_check_contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|check| check["frozen_inputs"].as_object())
        .flat_map(|inputs| inputs.keys().map(String::as_str))
        .for_each(|path| paths.push(path));
    intake["dotnet_check_contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|check| check["frozen_inputs"].as_object())
        .flat_map(|inputs| inputs.keys().map(String::as_str))
        .for_each(|path| paths.push(path));
    intake["ceo_source_files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|file| file["path"].as_str())
        .for_each(|path| paths.push(path));
    intake["angular_check_contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|check| check["frozen_inputs"].as_object())
        .flat_map(|inputs| inputs.keys().map(String::as_str))
        .for_each(|path| paths.push(path));
    let registry = crate::Registry::new();
    intake["ceo_source_inputs"]
        .as_array()
        .into_iter()
        .flatten()
        .for_each(|input| {
            paths.extend(
                registry.file_inputs(input["tool"].as_str().unwrap_or(""), &input["arguments"]),
            )
        });
    paths
}
pub(crate) fn protected(intake: &Value, path: &str) -> bool {
    paths(intake).contains(&path)
}
pub(crate) fn write_protected(
    root: &std::path::Path,
    intake: &Value,
    path: &str,
) -> anyhow::Result<bool> {
    let inputs = paths(intake);
    if inputs.contains(&path) {
        return Ok(true);
    }
    if inputs.is_empty() {
        return Ok(false);
    }
    let directory = match crate::workspace::open(root, false) {
        Ok(directory) => directory,
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
        {
            return Ok(false)
        }
        Err(error) => return Err(error),
    };
    for input in inputs {
        if directory.same_file(path, input)? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    #[test]
    fn angular_owner_controls_are_protected_through_hardlink_aliases() {
        let root =
            std::env::temp_dir().join(format!("gents-angular-owner-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(root.join("angular.json"), "{}").unwrap();
        std::fs::write(root.join("main.ts"), "source").unwrap();
        let intake = serde_json::json!({"angular_check_contracts":[{"frozen_inputs":{"angular.json":"frozen"}}]});
        assert!(super::write_protected(&root, &intake, "angular.json").unwrap());
        assert!(!super::write_protected(&root, &intake, "main.ts").unwrap());
        std::fs::hard_link(root.join("angular.json"), root.join("alias.json")).unwrap();
        assert!(super::write_protected(&root, &intake, "angular.json").unwrap());
        assert!(super::write_protected(&root, &intake, "alias.json").is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
