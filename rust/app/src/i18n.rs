//! Reader for the section/key language files used by the Go backend.

use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Default)]
pub struct Lang {
    sections: HashMap<String, HashMap<String, String>>,
}

impl Lang {
    pub fn load(path: &Path) -> Result<Self, String> {
        let mut parser = ini::configparser::ini::Ini::new();
        let parsed = parser
            .load(path.to_string_lossy().as_ref())
            .map_err(|error| format!("loading {}: {error}", path.display()))?;
        let sections = parsed
            .into_iter()
            .map(|(section, values)| {
                (
                    section,
                    values
                        .into_iter()
                        .filter_map(|(key, value)| value.map(|value| (key, value)))
                        .collect(),
                )
            })
            .collect();
        Ok(Self { sections })
    }

    pub fn get(&self, key: &str) -> &str {
        if key.is_empty() {
            return "";
        }
        let mut parts = key.splitn(2, '.');
        let section = parts.next().unwrap_or_default();
        let item = parts.next().unwrap_or("NOT EMPTY");
        let values = self
            .sections
            .get(section)
            .or_else(|| self.sections.get(&section.to_lowercase()));
        values
            .and_then(|values| values.get(&item.to_lowercase()))
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn get_and_insert(&self, key: &str, insert: &[&str]) -> String {
        let mut value = self.get(key).to_string();
        value.push(' ');
        value.push_str(&insert.join(""));
        value
    }
}

#[cfg(test)]
mod tests {
    use super::Lang;

    #[test]
    fn loads_sections_keys_and_go_fallback_lookup() {
        let path = std::env::temp_dir().join(format!("yin-lang-{}.ini", std::process::id()));
        std::fs::write(
            &path,
            "[lang_info]\nversion=1.0\nNOT EMPTY=required\n[errors]\nmissing=Not found\n",
        )
        .expect("write temporary language file");
        let lang = Lang::load(&path).expect("load language file");
        assert_eq!(lang.get("lang_info.version"), "1.0");
        assert_eq!(lang.get("lang_info"), "required");
        assert_eq!(lang.get("errors.missing"), "Not found");
        assert_eq!(lang.get("unknown.key"), "");
        assert_eq!(lang.get(""), "");
        assert_eq!(
            lang.get_and_insert("errors.missing", &[" ", "item"]),
            "Not found  item"
        );
        std::fs::remove_file(path).expect("remove temporary language file");
    }
}
