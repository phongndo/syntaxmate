use std::collections::BTreeMap;

use crate::engine::{
    grammar::{CompiledGrammar, load_dev_grammar_from_str},
    state::GrammarId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GrammarAsset {
    pub(crate) language: &'static str,
    pub(crate) scope_name: &'static str,
    pub(crate) source: &'static str,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RegistryError {
    UnknownLanguage(String),
    Parse { language: String, message: String },
}

#[allow(dead_code)]
#[derive(Debug, Default)]
pub(crate) struct GrammarRegistry {
    cache: BTreeMap<&'static str, CompiledGrammar>,
}

#[allow(dead_code)]
impl GrammarRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn available_languages() -> Vec<&'static str> {
        CORE_ASSETS.iter().map(|asset| asset.language).collect()
    }

    pub(crate) fn asset(language: &str) -> Option<&'static GrammarAsset> {
        CORE_ASSETS.iter().find(|asset| asset.language == language)
    }

    pub(crate) fn grammar(&mut self, language: &str) -> Result<&CompiledGrammar, RegistryError> {
        let asset = Self::asset(language)
            .ok_or_else(|| RegistryError::UnknownLanguage(language.to_owned()))?;
        if !self.cache.contains_key(asset.language) {
            let id = GrammarId(self.cache.len() as u16);
            let grammar = load_dev_grammar_from_str(id, asset.source).map_err(|error| {
                RegistryError::Parse {
                    language: asset.language.to_owned(),
                    message: error.to_string(),
                }
            })?;
            self.cache.insert(asset.language, grammar);
        }
        Ok(self
            .cache
            .get(asset.language)
            .expect("grammar inserted before lookup"))
    }
}

/// Curated core regression asset set used by legacy registry tests.
///
/// Order is stable and alphabetical by language id. Asset path stems that differ
/// from the test id (`bash`/`shellscript`) use the recovered filename under
/// `assets/grammars/languages/`.
pub(crate) const CORE_ASSETS: &[GrammarAsset] = &[
    GrammarAsset {
        language: "bash",
        scope_name: "source.shell",
        source: include_str!("../../assets/grammars/languages/shellscript.tmLanguage.json"),
    },
    GrammarAsset {
        language: "c",
        scope_name: "source.c",
        source: include_str!("../../assets/grammars/languages/c.tmLanguage.json"),
    },
    GrammarAsset {
        language: "cpp",
        scope_name: "source.cpp",
        source: include_str!("../../assets/grammars/languages/cpp.tmLanguage.json"),
    },
    GrammarAsset {
        language: "cpp-macro",
        scope_name: "source.cpp.embedded.macro",
        source: include_str!("../../assets/grammars/languages/cpp-macro.tmLanguage.json"),
    },
    GrammarAsset {
        language: "csharp",
        scope_name: "source.cs",
        source: include_str!("../../assets/grammars/languages/csharp.tmLanguage.json"),
    },
    GrammarAsset {
        language: "css",
        scope_name: "source.css",
        source: include_str!("../../assets/grammars/languages/css.tmLanguage.json"),
    },
    GrammarAsset {
        language: "dockerfile",
        scope_name: "source.dockerfile",
        source: include_str!("../../assets/grammars/languages/docker.tmLanguage.json"),
    },
    GrammarAsset {
        language: "go",
        scope_name: "source.go",
        source: include_str!("../../assets/grammars/languages/go.tmLanguage.json"),
    },
    GrammarAsset {
        language: "html",
        scope_name: "text.html.basic",
        source: include_str!("../../assets/grammars/languages/html.tmLanguage.json"),
    },
    GrammarAsset {
        language: "java",
        scope_name: "source.java",
        source: include_str!("../../assets/grammars/languages/java.tmLanguage.json"),
    },
    GrammarAsset {
        language: "javascript",
        scope_name: "source.js",
        source: include_str!("../../assets/grammars/languages/javascript.tmLanguage.json"),
    },
    GrammarAsset {
        language: "json",
        scope_name: "source.json",
        source: include_str!("../../assets/grammars/languages/json.tmLanguage.json"),
    },
    GrammarAsset {
        language: "jsx",
        scope_name: "source.js.jsx",
        source: include_str!("../../assets/grammars/languages/jsx.tmLanguage.json"),
    },
    GrammarAsset {
        language: "kotlin",
        scope_name: "source.kotlin",
        source: include_str!("../../assets/grammars/languages/kotlin.tmLanguage.json"),
    },
    GrammarAsset {
        language: "lua",
        scope_name: "source.lua",
        source: include_str!("../../assets/grammars/languages/lua.tmLanguage.json"),
    },
    GrammarAsset {
        language: "make",
        scope_name: "source.makefile",
        source: include_str!("../../assets/grammars/languages/make.tmLanguage.json"),
    },
    GrammarAsset {
        language: "markdown",
        scope_name: "text.html.markdown",
        source: include_str!("../../assets/grammars/languages/markdown.tmLanguage.json"),
    },
    GrammarAsset {
        language: "nix",
        scope_name: "source.nix",
        source: include_str!("../../assets/grammars/languages/nix.tmLanguage.json"),
    },
    GrammarAsset {
        language: "php",
        scope_name: "source.php",
        source: include_str!("../../assets/grammars/languages/php.tmLanguage.json"),
    },
    GrammarAsset {
        language: "powershell",
        scope_name: "source.powershell",
        source: include_str!("../../assets/grammars/languages/powershell.tmLanguage.json"),
    },
    GrammarAsset {
        language: "python",
        scope_name: "source.python",
        source: include_str!("../../assets/grammars/languages/python.tmLanguage.json"),
    },
    GrammarAsset {
        language: "ruby",
        scope_name: "source.ruby",
        source: include_str!("../../assets/grammars/languages/ruby.tmLanguage.json"),
    },
    GrammarAsset {
        language: "rust",
        scope_name: "source.rust",
        source: include_str!("../../assets/grammars/languages/rust.tmLanguage.json"),
    },
    GrammarAsset {
        language: "scss",
        scope_name: "source.css.scss",
        source: include_str!("../../assets/grammars/languages/scss.tmLanguage.json"),
    },
    GrammarAsset {
        language: "sql",
        scope_name: "source.sql",
        source: include_str!("../../assets/grammars/languages/sql.tmLanguage.json"),
    },
    GrammarAsset {
        language: "swift",
        scope_name: "source.swift",
        source: include_str!("../../assets/grammars/languages/swift.tmLanguage.json"),
    },
    GrammarAsset {
        language: "terraform",
        scope_name: "source.hcl.terraform",
        source: include_str!("../../assets/grammars/languages/terraform.tmLanguage.json"),
    },
    GrammarAsset {
        language: "toml",
        scope_name: "source.toml",
        source: include_str!("../../assets/grammars/languages/toml.tmLanguage.json"),
    },
    GrammarAsset {
        language: "tsx",
        scope_name: "source.tsx",
        source: include_str!("../../assets/grammars/languages/tsx.tmLanguage.json"),
    },
    GrammarAsset {
        language: "typescript",
        scope_name: "source.ts",
        source: include_str!("../../assets/grammars/languages/typescript.tmLanguage.json"),
    },
    GrammarAsset {
        language: "yaml",
        scope_name: "source.yaml",
        source: include_str!("../../assets/grammars/languages/yaml.tmLanguage.json"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_lists_core_assets_in_stable_order() {
        assert_eq!(GrammarRegistry::available_languages()[0], "bash");
        assert_eq!(
            GrammarRegistry::available_languages().len(),
            CORE_ASSETS.len()
        );
        assert!(GrammarRegistry::asset("json").is_some());
        assert!(GrammarRegistry::asset("dockerfile").is_some());
        assert!(GrammarRegistry::asset("cpp-macro").is_some());
        assert!(GrammarRegistry::asset("definitely-missing").is_none());
    }

    #[test]
    fn registry_lazily_loads_json_grammar() {
        let mut registry = GrammarRegistry::new();
        let grammar = registry.grammar("json").unwrap();
        assert_eq!(grammar.scope_name, "source.json");
        assert!(!grammar.top_level.is_empty());
    }

    #[test]
    fn registry_loads_all_core_assets() {
        let mut registry = GrammarRegistry::new();
        for asset in CORE_ASSETS {
            let grammar = registry
                .grammar(asset.language)
                .unwrap_or_else(|error| panic!("{} should load: {error:?}", asset.language));
            assert_eq!(grammar.scope_name, asset.scope_name);
        }
    }

    #[test]
    fn core_assets_cover_public_core_30_plus_cpp_macro() {
        let languages: Vec<_> = CORE_ASSETS.iter().map(|a| a.language).collect();
        for expected in [
            "rust",
            "c",
            "cpp",
            "csharp",
            "go",
            "python",
            "java",
            "kotlin",
            "swift",
            "ruby",
            "php",
            "lua",
            "javascript",
            "jsx",
            "typescript",
            "tsx",
            "bash",
            "powershell",
            "html",
            "css",
            "scss",
            "json",
            "yaml",
            "toml",
            "markdown",
            "sql",
            "dockerfile",
            "make",
            "nix",
            "terraform",
            "cpp-macro",
        ] {
            assert!(
                languages.contains(&expected),
                "missing core asset {expected}"
            );
        }
        assert_eq!(languages.len(), 31);
    }
}
