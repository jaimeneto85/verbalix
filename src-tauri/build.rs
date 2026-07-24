use std::{collections::HashMap, env, fs, path::PathBuf};

const CANONICAL_URL: &str = "VITE_SUPABASE_URL";
const CANONICAL_KEY: &str = "VITE_SUPABASE_ANON_KEY";
const LEGACY_URL: &str = "VERBALIX_SUPABASE_URL";
const LEGACY_KEY: &str = "VERBALIX_SUPABASE_ANON_KEY";

fn main() {
    for name in [CANONICAL_URL, CANONICAL_KEY, LEGACY_URL, LEGACY_KEY] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    println!("cargo:rerun-if-changed=../.env");

    let values = env_file_values();
    let canonical = process_pair(CANONICAL_URL, CANONICAL_KEY)
        .or_else(|| file_pair(&values, CANONICAL_URL, CANONICAL_KEY));
    let legacy =
        process_pair(LEGACY_URL, LEGACY_KEY).or_else(|| file_pair(&values, LEGACY_URL, LEGACY_KEY));
    write_embedded_config(canonical, legacy);

    tauri_build::build()
}

fn process_pair(url_name: &str, key_name: &str) -> Option<(String, String)> {
    complete_pair(env::var(url_name).ok(), env::var(key_name).ok())
}

fn file_pair(
    values: &HashMap<String, String>,
    url_name: &str,
    key_name: &str,
) -> Option<(String, String)> {
    complete_pair(values.get(url_name).cloned(), values.get(key_name).cloned())
}

fn complete_pair(url: Option<String>, key: Option<String>) -> Option<(String, String)> {
    let url = url?.trim().to_owned();
    let key = key?.trim().to_owned();
    (!url.is_empty()
        && !key.is_empty()
        && !url.contains('\r')
        && !url.contains('\n')
        && !key.contains('\r')
        && !key.contains('\n'))
    .then_some((url, key))
}

fn env_file_values() -> HashMap<String, String> {
    let path = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default()).join("../.env");
    dotenvy::from_path_iter(path)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .collect()
}

fn write_embedded_config(canonical: Option<(String, String)>, legacy: Option<(String, String)>) {
    let (canonical_url, canonical_key) = split_pair(canonical);
    let (legacy_url, legacy_key) = split_pair(legacy);
    let contents = format!(
        "pub const VITE_URL: Option<&str> = {};\n\
         pub const VITE_KEY: Option<&str> = {};\n\
         pub const LEGACY_URL: Option<&str> = {};\n\
         pub const LEGACY_KEY: Option<&str> = {};\n",
        rust_option(canonical_url.as_deref()),
        rust_option(canonical_key.as_deref()),
        rust_option(legacy_url.as_deref()),
        rust_option(legacy_key.as_deref()),
    );
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap_or_default())
        .join("verbalix_backend_config.rs");
    fs::write(output, contents).expect("failed to write embedded public backend configuration");
}

fn split_pair(pair: Option<(String, String)>) -> (Option<String>, Option<String>) {
    pair.map_or((None, None), |(url, key)| (Some(url), Some(key)))
}

fn rust_option(value: Option<&str>) -> String {
    value.map_or_else(|| "None".to_owned(), |value| format!("Some({value:?})"))
}
