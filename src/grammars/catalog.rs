//! Curated language, extension, and basename aliases for the in-house bundle.

use std::collections::BTreeSet;

pub const LANGUAGE_ALIASES: &[(&str, &str)] = &[
    ("1c", "bsl"),
    ("actionscript-3", "actionscript"),
    ("adoc", "asciidoc-asciidoctor"),
    ("apache", "apache-conf"),
    ("apl", "apl"),
    ("asciidoc", "asciidoc-asciidoctor"),
    ("bzl", "starlark"),
    ("bazel", "starlark"),
    ("bat", "batch-file"),
    ("batch", "batch-file"),
    ("be", "berry"),
    ("bib", "bibtex"),
    ("bird", "bird2"),
    ("c++", "cpp"),
    ("cbl", "cobol"),
    ("cc", "cpp"),
    ("c#", "csharp"),
    ("clar", "clarity"),
    ("clj", "clojure"),
    ("cljs", "clojure"),
    ("cmd", "batch-file"),
    ("cob", "cobol"),
    ("coffee", "coffeescript"),
    ("cjs", "javascript"),
    ("cl", "common-lisp"),
    ("code-owner", "codeowners"),
    ("codeowner", "codeowners"),
    ("common-lisp", "common-lisp"),
    ("commonlisp", "common-lisp"),
    ("coq", "coq"),
    ("csv", "comma-separated-values"),
    ("cs", "csharp"),
    ("cu", "cuda"),
    ("cuh", "cuda"),
    ("cxx", "cpp"),
    ("cls", "apex"),
    ("dm", "dream-maker"),
    ("dme", "dream-maker"),
    ("dmm", "dream-maker"),
    ("docker", "dockerfile"),
    ("dockerfile", "docker"),
    ("dockerignore", "ignore"),
    ("edn", "clojure"),
    ("el", "emacs-lisp"),
    ("elisp", "emacs-lisp"),
    ("emacs-lisp", "emacs-lisp"),
    ("env", "dotenv"),
    ("ejs", "ejs"),
    ("erb", "html-rails"),
    ("ex", "elixir"),
    ("exs", "elixir"),
    ("f77", "fortran-fixed-form"),
    ("f90", "fortran-modern"),
    ("f95", "fortran-modern"),
    ("fortran", "fortran-modern"),
    ("fortran-free-form", "fortran-modern"),
    ("fs", "f#"),
    ("fsharp", "f#"),
    ("fnl", "fennel"),
    ("feature", "gherkin"),
    ("ftl", "fluent"),
    ("gjs", "glimmer-js"),
    ("gdscript", "gdscript-godot-engine"),
    ("gts", "glimmer-ts"),
    ("git-ignore", "ignore"),
    ("git-rebase", "git-rebase-todo"),
    ("gql", "graphql"),
    ("graphqls", "graphql"),
    ("gradle", "groovy"),
    ("haml", "ruby-haml"),
    ("hcl", "terraform"),
    ("hjson", "json"),
    ("hlsl", "hlsl"),
    ("hs", "haskell"),
    ("hx", "haxe"),
    ("hy", "hy"),
    ("html-derivative", "html"),
    ("jinja", "jinja2"),
    ("jinja-html", "html-jinja2"),
    ("jl", "julia"),
    ("ipynb", "json"),
    ("kql", "kusto"),
    ("kt", "kotlin"),
    ("kts", "kotlin"),
    ("json5", "json"),
    ("jsonc", "json"),
    ("jsonl", "json"),
    ("jisonlex", "jison"),
    ("automount", "systemd"),
    ("just", "just"),
    ("justfile", "just"),
    ("lean", "lean-4"),
    ("lhs", "literate-haskell"),
    ("liquid", "liquid"),
    ("ll", "llvm"),
    ("lsp", "common-lisp"),
    ("md", "markdown"),
    ("mdx", "mdx"),
    ("ml", "ocaml"),
    ("mli", "ocaml"),
    ("mjs", "javascript"),
    ("mlir", "mlir"),
    ("mips", "mipsasm"),
    ("mount", "systemd"),
    ("msg", "rosmsg"),
    ("nb", "wolfram"),
    ("ndjson", "json"),
    ("gitignore", "ignore"),
    ("ignorefile", "ignore"),
    ("js", "javascript"),
    ("javascript-babel", "jsx"),
    ("jsx", "jsx"),
    ("objective-cpp", "objective-c++"),
    ("node", "javascript"),
    ("objc", "objective-c"),
    ("objc++", "objective-c++"),
    ("pb", "protocol-buffer"),
    ("pbt", "protocol-buffer-text"),
    ("pot", "po"),
    ("pro", "prolog"),
    ("prolog", "prolog"),
    ("plsql", "sql"),
    ("postgres", "sql"),
    ("postgresql", "sql"),
    ("postcss", "css"),
    ("ql", "codeql"),
    ("properties", "java-properties"),
    ("proto", "protocol-buffer"),
    ("protobuf", "protocol-buffer"),
    ("ps1", "powershell"),
    ("psm1", "powershell"),
    ("psd1", "powershell"),
    ("ps", "powershell"),
    ("pwsh", "powershell"),
    ("python3", "python"),
    ("py", "python"),
    ("rb", "ruby"),
    ("regex", "regular-expression"),
    ("regexp", "regular-expression"),
    ("risc-v", "riscv"),
    ("rest", "restructuredtext"),
    ("rst", "restructuredtext"),
    ("rs", "rust"),
    ("scad", "openscad"),
    ("s", "asm"),
    ("scm", "scheme"),
    ("scheme", "scheme"),
    ("makefile", "make"),
    ("bash", "shellscript"),
    ("shell", "shellscript"),
    ("shell-session", "shell-unix-generic"),
    ("shellsession", "shell-unix-generic"),
    ("sh", "shellscript"),
    ("shader", "shaderlab"),
    ("slim", "ruby-slim"),
    ("sol", "solidity"),
    ("spl", "splunk"),
    ("spir-v", "spirv"),
    ("spirv-asm", "spirv"),
    ("sv", "systemverilog"),
    ("service", "systemd"),
    ("socket", "systemd"),
    ("scope", "systemd"),
    ("slice", "systemd"),
    ("srv", "rosmsg"),
    ("swap", "systemd"),
    ("system-verilog", "systemverilog"),
    ("target", "systemd"),
    ("td", "tablegen"),
    ("tfstate", "json"),
    ("tf", "terraform"),
    ("tfvars", "terraform"),
    ("ts", "typescript"),
    ("trigger", "apex"),
    ("tres", "gdresource"),
    ("tscn", "gdresource"),
    ("tsv", "tab-separated-values"),
    ("ttl", "turtle"),
    ("timer", "systemd"),
    ("twig", "twig"),
    ("typescriptreact", "tsx"),
    ("vim", "viml"),
    ("vimscript", "viml"),
    ("vue", "vue-component"),
    ("vue-html", "vue-component"),
    ("vue-vine", "vue-component"),
    ("wast", "wasm"),
    ("wat", "wasm"),
    ("wy", "wenyan"),
    ("wl", "wolfram"),
    ("wls", "wolfram"),
    ("x86asm", "x86-64-assembly"),
    ("xslt", "xsl"),
    ("yml", "yaml"),
    ("zs", "zenscript"),
    ("zsh", "shellscript"),
];

pub const EXTENSION_ALIASES: &[(&str, &str)] = &[
    ("bazel", "starlark"),
    ("bzl", "starlark"),
    ("c++", "cpp"),
    ("cc", "cpp"),
    ("cjs", "javascript"),
    ("cls", "apex"),
    // Both Apache and BIRD publish `conf`; generic .conf files follow the
    // broadly used Apache configuration grammar rather than catalog order.
    ("conf", "apache"),
    ("cs", "csharp"),
    ("cu", "cuda"),
    ("cuh", "cuda"),
    ("cxx", "cpp"),
    ("fs", "f#"),
    ("go", "go"),
    ("h", "c"),
    ("h++", "cpp"),
    ("hh", "cpp"),
    ("hpp", "cpp"),
    ("hxx", "cpp"),
    ("java", "java"),
    ("js", "javascript"),
    ("jsx", "jsx"),
    ("kt", "kotlin"),
    ("kts", "kotlin"),
    ("lua", "lua"),
    ("md", "markdown"),
    ("mjs", "javascript"),
    ("mts", "typescript"),
    ("nix", "nix"),
    ("php", "php"),
    ("ps1", "powershell"),
    ("psm1", "powershell"),
    ("psd1", "powershell"),
    ("py", "python"),
    ("rb", "ruby"),
    ("rs", "rust"),
    ("cts", "typescript"),
    ("scss", "scss"),
    ("sh", "shellscript"),
    ("sql", "sql"),
    ("sv", "systemverilog"),
    ("swift", "swift"),
    ("tf", "terraform"),
    ("tfvars", "terraform"),
    // VS Code assigns .tex files to its LaTeX language contribution. Keep
    // plain `tex` available only as an explicit language override.
    ("tex", "latex"),
    ("toml", "toml"),
    ("ts", "typescript"),
    ("tsx", "tsx"),
    ("v", "verilog"),
    ("yaml", "yaml"),
    ("yml", "yaml"),
];

/// Explicit ownership for every extension that otherwise has multiple public
/// catalog owners. An empty owner deliberately suppresses a file type that is
/// only advertised by embedded/tag fragment grammars.
pub const EXTENSION_PRECEDENCE: &[(&str, &str)] = &[
    ("asm", "asm"),
    ("bib", "bibtex"),
    ("cl", "opencl"),
    ("conf", "apache"),
    ("gs", "genie"),
    ("hcl", "terraform"),
    ("hjson", "json"),
    ("html", "html"),
    ("html-derivative", "html-derivative"),
    ("js", "javascript"),
    ("json5", "json5"),
    ("jsonc", "jsonc"),
    ("jsonl", "jsonl"),
    ("m", "matlab"),
    ("plsql", "plsql"),
    ("postcss", "postcss"),
    ("pp", "puppet"),
    ("res", ""),
    ("s", "asm"),
    ("ss", "scheme"),
    ("svelte", "svelte"),
    ("v", "verilog"),
    ("vh", "verilog"),
    ("vsh", "glsl"),
    ("vue", "vue"),
];

pub const BASENAME_ALIASES: &[(&str, &str)] = &[
    ("BUILD", "starlark"),
    ("BUILD.bazel", "starlark"),
    ("WORKSPACE", "starlark"),
    ("WORKSPACE.bazel", "starlark"),
    ("MODULE.bazel", "starlark"),
    ("CODEOWNERS", "codeowners"),
    ("Dockerfile", "dockerfile"),
    ("Dockerfile", "docker"),
    ("Justfile", "just"),
    ("qmldir", "qmldir"),
    ("Makefile", "make"),
    ("GNUmakefile", "make"),
    ("BSDmakefile", "make"),
    ("CMakeLists.txt", "cmake"),
    (".clang-format", "yaml"),
    (".clang-tidy", "yaml"),
    (".dockerignore", "ignore"),
    (".eslintignore", "ignore"),
    (".git-blame-ignore-revs", "ignore"),
    (".gitignore", "ignore"),
    (".npmignore", "ignore"),
    (".prettierignore", "ignore"),
    (".stylelintignore", "ignore"),
    (".vercelignore", "ignore"),
];

pub fn aliases_for_language(language: &str) -> Vec<String> {
    LANGUAGE_ALIASES
        .iter()
        .filter_map(|(alias, target)| (*target == language).then_some(normalize(alias)))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn extensions_for_language(language: &str) -> Vec<String> {
    let mut extensions = BTreeSet::new();
    if extension_is_allowed(language, language) {
        extensions.insert(normalize(language));
    }
    for (alias, target) in LANGUAGE_ALIASES {
        if *target == language && extension_is_allowed(alias, language) {
            extensions.insert(normalize(alias));
        }
    }
    for (extension, target) in EXTENSION_ALIASES {
        if *target == language {
            extensions.insert(normalize(extension));
        }
    }
    extensions.into_iter().collect()
}

pub fn extension_override(extension: &str) -> Option<&'static str> {
    if let Some(target) = extension_precedence(extension) {
        return target;
    }
    EXTENSION_ALIASES
        .iter()
        .find_map(|(candidate, target)| (*candidate == extension).then_some(*target))
}

pub fn extension_precedence(extension: &str) -> Option<Option<&'static str>> {
    EXTENSION_PRECEDENCE.iter().find_map(|(candidate, target)| {
        (*candidate == extension).then_some((!target.is_empty()).then_some(*target))
    })
}

pub fn extension_is_allowed(extension: &str, language: &str) -> bool {
    match extension_precedence(extension) {
        Some(Some(target)) => target == language,
        Some(None) => false,
        None => extension_override(extension).is_none_or(|target| target == language),
    }
}

pub fn basenames_for_language(language: &str) -> Vec<String> {
    BASENAME_ALIASES
        .iter()
        .filter_map(|(basename, target)| (*target == language).then_some((*basename).to_owned()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn normalize_language_token(token: &str) -> String {
    normalize(token)
}

fn normalize(token: &str) -> String {
    token.trim().trim_start_matches('.').to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curated_extension_overrides_win_over_generated_file_types() {
        assert_eq!(extension_override("conf"), Some("apache"));
        assert!(extensions_for_language("apache").contains(&"conf".to_owned()));
        assert!(!extensions_for_language("bird2").contains(&"conf".to_owned()));

        assert!(!extensions_for_language("v").contains(&"v".to_owned()));
        assert!(extensions_for_language("verilog").contains(&"v".to_owned()));

        assert_eq!(extension_precedence("cl"), Some(Some("opencl")));
        assert_eq!(extension_override("js"), Some("javascript"));
        assert!(extensions_for_language("javascript").contains(&"js".to_owned()));
        assert!(extensions_for_language("c").contains(&"h".to_owned()));
        assert_eq!(extension_precedence("res"), Some(None));
    }

    #[test]
    fn curated_basenames_are_case_preserved() {
        assert!(basenames_for_language("starlark").contains(&"BUILD".to_owned()));
        assert!(basenames_for_language("dockerfile").contains(&"Dockerfile".to_owned()));
        assert!(basenames_for_language("make").contains(&"Makefile".to_owned()));
        assert!(basenames_for_language("ignore").contains(&".gitignore".to_owned()));
        assert!(basenames_for_language("ignore").contains(&".dockerignore".to_owned()));
    }

    #[test]
    fn core_30_aliases_and_extensions() {
        assert!(aliases_for_language("shellscript").contains(&"bash".to_owned()));
        assert!(aliases_for_language("shellscript").contains(&"sh".to_owned()));
        assert!(aliases_for_language("dockerfile").contains(&"docker".to_owned()));
        assert!(aliases_for_language("ignore").contains(&"git-ignore".to_owned()));
        assert!(aliases_for_language("rust").contains(&"rs".to_owned()));
        assert!(aliases_for_language("typescript").contains(&"ts".to_owned()));
        assert!(extensions_for_language("python").contains(&"py".to_owned()));
        assert!(extensions_for_language("ruby").contains(&"rb".to_owned()));
        assert!(extensions_for_language("kotlin").contains(&"kt".to_owned()));
        assert!(extensions_for_language("terraform").contains(&"tf".to_owned()));
        assert!(extensions_for_language("powershell").contains(&"ps1".to_owned()));
    }
}
