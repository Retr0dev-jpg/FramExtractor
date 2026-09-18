use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::OnceLock,
};

const EMBED_EN: &str = include_str!("../lang/en.json");
const EMBED_IT: &str = include_str!("../lang/it.json");

static LANG: OnceLock<HashMap<String, String>> = OnceLock::new();

pub fn init() {
    let _ = LANG.set(load());
}

pub fn t(key: &str) -> String {
    LANG.get()
        .and_then(|map| map.get(key).cloned())
        .unwrap_or_else(|| key.to_string())
}

pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    let mut out = t(key);
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

fn load() -> HashMap<String, String> {
    let mut map = parse_json(&read_or_embed("en", EMBED_EN));
    if is_italian() {
        for (key, value) in parse_json(&read_or_embed("it", EMBED_IT)) {
            map.insert(key, value);
        }
    }
    map
}

fn is_italian() -> bool {
    sys_locale::get_locale()
        .map(|locale| {
            let locale = locale.to_ascii_lowercase();
            locale == "it" || locale.starts_with("it-") || locale.starts_with("it_")
        })
        .unwrap_or(false)
}

fn read_or_embed(code: &str, embedded: &str) -> String {
    for dir in lang_dirs() {
        let path = dir.join(format!("{code}.json"));
        if let Ok(text) = fs::read_to_string(path) {
            return text;
        }
    }
    embedded.to_string()
}

fn lang_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("lang"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("lang"));
    }
    dirs
}

fn parse_json(text: &str) -> HashMap<String, String> {
    serde_json::from_str(text).unwrap_or_default()
}
