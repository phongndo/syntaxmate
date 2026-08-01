use std::{collections::HashMap, sync::Arc};

use super::hashing::{self, FastMap};

use crate::SyntaxClass;

use super::state::{ScopeId, ScopeStackId};

#[derive(Debug, Clone, Default)]
pub struct ScopeInterner {
    // The id map and every exported scope table share the same allocation.
    // Scope names used to be copied into both `names` and `ids`, then copied
    // again for every highlighting result produced by a tokenizer.
    names: Vec<Arc<str>>,
    ids: HashMap<Arc<str>, ScopeId>,
    classes: Vec<Option<SyntaxClass>>,
}

impl ScopeInterner {
    pub fn intern(&mut self, name: &str) -> ScopeId {
        if let Some(id) = self.ids.get(name) {
            return *id;
        }
        let id = ScopeId(self.names.len() as u32);
        let name: Arc<str> = Arc::from(name);
        self.names.push(Arc::clone(&name));
        self.ids.insert(name, id);
        self.classes
            .push(classify_scope_name(&self.names[id.0 as usize]));
        id
    }

    pub fn get(&self, id: ScopeId) -> Option<&str> {
        self.names.get(id.0 as usize).map(AsRef::as_ref)
    }

    pub(crate) fn get_arc(&self, id: ScopeId) -> Option<Arc<str>> {
        self.names.get(id.0 as usize).map(Arc::clone)
    }

    pub fn class(&self, id: ScopeId) -> Option<SyntaxClass> {
        self.classes.get(id.0 as usize).copied().flatten()
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeTemplateId(pub u32);

#[derive(Debug, Clone, Default)]
pub struct ScopeTemplateInterner {
    templates: Vec<Arc<[ScopeId]>>,
    ids: HashMap<Arc<[ScopeId]>, ScopeTemplateId>,
    scope_text_ids: HashMap<String, ScopeTemplateId>,
    prefix_text_ids: HashMap<String, ScopeTemplateId>,
}

impl ScopeTemplateInterner {
    pub fn intern_scope_template(
        &mut self,
        text: &str,
        scopes: &mut ScopeInterner,
    ) -> ScopeTemplateId {
        if let Some(id) = self.scope_text_ids.get(text) {
            return *id;
        }
        let atoms = text
            .split_whitespace()
            .filter_map(normalize_scope_atom)
            .map(|atom| scopes.intern(&atom))
            .collect::<Vec<_>>();
        let id = self.intern_atoms(atoms);
        self.scope_text_ids.insert(text.to_owned(), id);
        id
    }

    pub fn intern_prefix_template(
        &mut self,
        text: &str,
        scopes: &mut ScopeInterner,
    ) -> ScopeTemplateId {
        if let Some(id) = self.prefix_text_ids.get(text) {
            return *id;
        }
        let atoms = text
            .split_whitespace()
            .map(|atom| scopes.intern(atom))
            .collect::<Vec<_>>();
        let id = self.intern_atoms(atoms);
        self.prefix_text_ids.insert(text.to_owned(), id);
        id
    }

    pub fn get(&self, id: ScopeTemplateId) -> Option<&[ScopeId]> {
        self.templates.get(id.0 as usize).map(AsRef::as_ref)
    }

    pub fn len(&self) -> usize {
        self.templates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    fn intern_atoms(&mut self, atoms: Vec<ScopeId>) -> ScopeTemplateId {
        if let Some(id) = self.ids.get(atoms.as_slice()) {
            return *id;
        }
        let id = ScopeTemplateId(self.templates.len() as u32);
        let atoms = Arc::<[ScopeId]>::from(atoms);
        self.templates.push(Arc::clone(&atoms));
        self.ids.insert(atoms, id);
        id
    }
}

fn normalize_scope_atom(scope: &str) -> Option<String> {
    if !scope.starts_with('.') && !scope.ends_with('.') && !scope.contains("..") {
        return Some(scope.to_owned());
    }
    let normalized = scope
        .split('.')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>()
        .join(".");
    (!normalized.is_empty()).then_some(normalized)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopeStackNode {
    pub parent: ScopeStackId,
    pub scope: Option<ScopeId>,
    pub class: Option<SyntaxClass>,
    pub hash: u64,
}

#[derive(Debug, Clone)]
pub struct ScopeStackInterner {
    nodes: Vec<ScopeStackNode>,
    edges: FastMap<(ScopeStackId, ScopeId), ScopeStackId>,
    template_edges: FastMap<(ScopeStackId, ScopeTemplateId), ScopeStackId>,
    template_once_edges: FastMap<(ScopeStackId, ScopeTemplateId), ScopeStackId>,
}

impl Default for ScopeStackInterner {
    fn default() -> Self {
        Self {
            nodes: vec![ScopeStackNode {
                parent: ScopeStackId(0),
                scope: None,
                class: None,
                hash: 0xcbf2_9ce4_8422_2325,
            }],
            edges: hashing::fast_map(),
            template_edges: hashing::fast_map(),
            template_once_edges: hashing::fast_map(),
        }
    }
}

impl ScopeStackInterner {
    pub fn empty(&self) -> ScopeStackId {
        ScopeStackId(0)
    }

    pub fn push(
        &mut self,
        parent: ScopeStackId,
        scope: ScopeId,
        scopes: &ScopeInterner,
    ) -> ScopeStackId {
        if let Some(id) = self.edges.get(&(parent, scope)) {
            return *id;
        }
        let parent_node = self
            .nodes
            .get(parent.0 as usize)
            .copied()
            .unwrap_or(self.nodes[0]);
        let scope_class = scopes.class(scope);
        let class = match (parent_node.class, scope_class) {
            (Some(SyntaxClass::Tag), _) | (_, Some(SyntaxClass::Tag)) => Some(SyntaxClass::Tag),
            (Some(SyntaxClass::Attribute), _) | (_, Some(SyntaxClass::Attribute)) => {
                Some(SyntaxClass::Attribute)
            }
            (parent, scope) => scope.or(parent),
        };
        let hash = (parent_node.hash ^ u64::from(scope.0)).wrapping_mul(0x0000_0100_0000_01b3);
        let id = ScopeStackId(self.nodes.len() as u32);
        self.nodes.push(ScopeStackNode {
            parent,
            scope: Some(scope),
            class,
            hash,
        });
        self.edges.insert((parent, scope), id);
        id
    }

    pub fn push_template(
        &mut self,
        mut parent: ScopeStackId,
        template: ScopeTemplateId,
        templates: &ScopeTemplateInterner,
        scopes: &ScopeInterner,
    ) -> ScopeStackId {
        let transition = (parent, template);
        if let Some(id) = self.template_edges.get(&transition) {
            return *id;
        }
        for scope in templates.get(template).unwrap_or_default() {
            parent = self.push(parent, *scope, scopes);
        }
        self.template_edges.insert(transition, parent);
        parent
    }

    pub fn push_template_once(
        &mut self,
        mut parent: ScopeStackId,
        template: ScopeTemplateId,
        templates: &ScopeTemplateInterner,
        scopes: &ScopeInterner,
    ) -> ScopeStackId {
        let transition = (parent, template);
        if let Some(id) = self.template_once_edges.get(&transition) {
            return *id;
        }
        for scope in templates.get(template).unwrap_or_default() {
            if self.top_scope(parent) != Some(*scope) {
                parent = self.push(parent, *scope, scopes);
            }
        }
        self.template_once_edges.insert(transition, parent);
        parent
    }

    pub fn parent(&self, id: ScopeStackId) -> ScopeStackId {
        self.nodes
            .get(id.0 as usize)
            .map_or(self.empty(), |node| node.parent)
    }

    pub fn top_scope(&self, id: ScopeStackId) -> Option<ScopeId> {
        self.nodes.get(id.0 as usize).and_then(|node| node.scope)
    }

    pub fn class(&self, id: ScopeStackId) -> Option<SyntaxClass> {
        self.nodes.get(id.0 as usize).and_then(|node| node.class)
    }

    pub fn hash(&self, id: ScopeStackId) -> u64 {
        self.nodes.get(id.0 as usize).map_or(0, |node| node.hash)
    }

    pub fn resolve(&self, id: ScopeStackId, scopes: &ScopeInterner) -> Vec<String> {
        self.resolve_ids(id)
            .into_iter()
            .filter_map(|scope| scopes.get(scope).map(str::to_owned))
            .collect()
    }

    pub fn resolve_ids(&self, id: ScopeStackId) -> Vec<ScopeId> {
        let mut ids = Vec::new();
        let mut cursor = id;
        while cursor != self.empty() {
            let Some(node) = self.nodes.get(cursor.0 as usize) else {
                break;
            };
            if let Some(scope) = node.scope {
                ids.push(scope);
            }
            cursor = node.parent;
        }
        ids.reverse();
        ids
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.len() == 1
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScopeClassifier {
    scope_cache: HashMap<String, Option<SyntaxClass>>,
    stack_cache: HashMap<Vec<String>, Option<SyntaxClass>>,
}

impl ScopeClassifier {
    pub fn class_for_stack(&mut self, stack: &[String]) -> Option<SyntaxClass> {
        if let Some(class) = self.stack_cache.get(stack) {
            return *class;
        }
        let class = classify_scope_stack(stack);
        self.stack_cache.insert(stack.to_vec(), class);
        class
    }

    pub fn class_for_scope(&mut self, scope: &str) -> Option<SyntaxClass> {
        if let Some(class) = self.scope_cache.get(scope) {
            return *class;
        }
        let class = classify_scope_name(scope);
        self.scope_cache.insert(scope.to_owned(), class);
        class
    }
}

pub fn classify_scope_stack(stack: &[String]) -> Option<SyntaxClass> {
    for preferred in [SyntaxClass::Tag, SyntaxClass::Attribute] {
        if stack
            .iter()
            .rev()
            .any(|scope| classify_scope_name(scope) == Some(preferred))
        {
            return Some(preferred);
        }
    }

    stack
        .iter()
        .rev()
        .find_map(|scope| classify_scope_name(scope))
}

pub fn classify_scope_name(scope: &str) -> Option<SyntaxClass> {
    let first = scope.split('.').next().unwrap_or(scope);
    match first {
        "comment" => Some(SyntaxClass::Comment),
        "string" => Some(SyntaxClass::String),
        "constant" => {
            if scope.starts_with("constant.numeric") {
                Some(SyntaxClass::Number)
            } else if scope.starts_with("constant.language.boolean") {
                Some(SyntaxClass::Keyword)
            } else {
                Some(SyntaxClass::Constant)
            }
        }
        "keyword" => {
            if scope.starts_with("keyword.operator") {
                Some(SyntaxClass::Operator)
            } else {
                Some(SyntaxClass::Keyword)
            }
        }
        "storage" => Some(SyntaxClass::Keyword),
        "variable" => {
            if scope.starts_with("variable.language")
                || scope.starts_with("variable.other.constant")
                || scope.starts_with("variable.other.enummember")
            {
                Some(SyntaxClass::Constant)
            } else if scope.starts_with("variable.other.property")
                || scope.starts_with("variable.other.member")
                || scope.starts_with("variable.other.object.property")
            {
                Some(SyntaxClass::Property)
            } else {
                Some(SyntaxClass::Variable)
            }
        }
        "support" => {
            if scope.starts_with("support.function") {
                Some(SyntaxClass::Function)
            } else if scope.starts_with("support.type") || scope.starts_with("support.class") {
                Some(SyntaxClass::Type)
            } else if scope.starts_with("support.constant") {
                Some(SyntaxClass::Constant)
            } else {
                None
            }
        }
        "entity" => {
            if scope.starts_with("entity.name.function") {
                Some(SyntaxClass::Function)
            } else if scope.starts_with("entity.name.type")
                || scope.starts_with("entity.name.class")
                || scope.starts_with("entity.name.struct")
                || scope.starts_with("entity.name.enum")
                || scope.starts_with("entity.name.trait")
            {
                Some(SyntaxClass::Type)
            } else if scope.starts_with("entity.name.tag") {
                Some(SyntaxClass::Tag)
            } else if scope.starts_with("entity.name.namespace") {
                Some(SyntaxClass::Module)
            } else if scope.starts_with("entity.name.label") {
                Some(SyntaxClass::Label)
            } else if scope.starts_with("entity.other.attribute-name") {
                Some(SyntaxClass::Attribute)
            } else {
                None
            }
        }
        "punctuation" => Some(SyntaxClass::Punctuation),
        "invalid" => Some(SyntaxClass::Keyword),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_scopes() {
        assert_eq!(
            classify_scope_name("keyword.control"),
            Some(SyntaxClass::Keyword)
        );
        assert_eq!(
            classify_scope_name("entity.name.function"),
            Some(SyntaxClass::Function)
        );
        assert_eq!(classify_scope_name("typewriter"), None);
    }

    #[test]
    fn classifies_variable_subkinds() {
        assert_eq!(
            classify_scope_name("variable.other.rust"),
            Some(SyntaxClass::Variable)
        );
        assert_eq!(
            classify_scope_name("variable.parameter.function.language.python"),
            Some(SyntaxClass::Variable)
        );
        assert_eq!(
            classify_scope_name("variable.language.self.rust"),
            Some(SyntaxClass::Constant)
        );
        assert_eq!(
            classify_scope_name("variable.other.constant.ruby"),
            Some(SyntaxClass::Constant)
        );
        assert_eq!(
            classify_scope_name("variable.other.enummember.cpp"),
            Some(SyntaxClass::Constant)
        );
        assert_eq!(
            classify_scope_name("variable.other.property.ts"),
            Some(SyntaxClass::Property)
        );
        assert_eq!(
            classify_scope_name("variable.other.member.cpp"),
            Some(SyntaxClass::Property)
        );
        assert_eq!(
            classify_scope_name("variable.other.object.property.js"),
            Some(SyntaxClass::Property)
        );
    }

    #[test]
    fn tag_and_attribute_have_priority() {
        let stack = vec![
            "source.test".to_owned(),
            "string.quoted".to_owned(),
            "entity.name.tag.html".to_owned(),
        ];
        assert_eq!(classify_scope_stack(&stack), Some(SyntaxClass::Tag));
    }

    #[test]
    fn scope_interner_deduplicates_and_caches_classes() {
        let mut scopes = ScopeInterner::default();
        let first = scopes.intern("keyword.control");
        let second = scopes.intern("keyword.control");
        assert_eq!(first, second);
        assert_eq!(scopes.len(), 1);
        assert_eq!(scopes.get(first), Some("keyword.control"));
        assert_eq!(scopes.class(first), Some(SyntaxClass::Keyword));
    }

    #[test]
    fn templates_preserve_scope_and_prefix_normalization_rules() {
        let mut scopes = ScopeInterner::default();
        let mut templates = ScopeTemplateInterner::default();
        let regular =
            templates.intern_scope_template(".entity..name.  keyword.control", &mut scopes);
        let prefix = templates.intern_prefix_template(".entity..name.", &mut scopes);

        let regular_names = templates
            .get(regular)
            .unwrap()
            .iter()
            .map(|id| scopes.get(*id).unwrap())
            .collect::<Vec<_>>();
        let prefix_names = templates
            .get(prefix)
            .unwrap()
            .iter()
            .map(|id| scopes.get(*id).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(regular_names, ["entity.name", "keyword.control"]);
        assert_eq!(prefix_names, [".entity..name."]);
    }

    #[test]
    fn persistent_stacks_deduplicate_edges_and_resolve_exactly() {
        let mut scopes = ScopeInterner::default();
        let source = scopes.intern("source.test");
        let keyword = scopes.intern("keyword.control");
        let mut stacks = ScopeStackInterner::default();
        let root = stacks.push(stacks.empty(), source, &scopes);
        let first = stacks.push(root, keyword, &scopes);
        let second = stacks.push(root, keyword, &scopes);

        assert_eq!(first, second);
        assert_eq!(
            stacks.resolve(first, &scopes),
            ["source.test", "keyword.control"]
        );
        assert_eq!(stacks.class(first), Some(SyntaxClass::Keyword));
        assert_eq!(stacks.parent(first), root);
        assert_ne!(stacks.hash(first), stacks.hash(root));
    }

    #[test]
    fn persistent_stack_class_matches_public_classifier_priority() {
        let stacks = [
            vec![
                "source.test",
                "string.quoted",
                "entity.other.attribute-name",
            ],
            vec![
                "source.test",
                "entity.other.attribute-name",
                "entity.name.tag.html",
                "string.quoted",
            ],
            vec!["source.test", "comment.line", "constant.numeric"],
        ];
        for names in stacks {
            let mut scopes = ScopeInterner::default();
            let mut interner = ScopeStackInterner::default();
            let mut stack = interner.empty();
            for name in &names {
                let scope = scopes.intern(name);
                stack = interner.push(stack, scope, &scopes);
            }
            let public = names
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>();
            assert_eq!(interner.class(stack), classify_scope_stack(&public));
        }
    }

    #[test]
    fn push_template_once_only_suppresses_the_current_top() {
        let mut scopes = ScopeInterner::default();
        let mut templates = ScopeTemplateInterner::default();
        let source = scopes.intern("source.test");
        let prefix = templates.intern_prefix_template("source.test meta.inner", &mut scopes);
        let mut stacks = ScopeStackInterner::default();
        let root = stacks.push(stacks.empty(), source, &scopes);
        let stack = stacks.push_template_once(root, prefix, &templates, &scopes);
        assert_eq!(
            stacks.resolve(stack, &scopes),
            ["source.test", "meta.inner"]
        );
    }

    #[test]
    fn whole_template_transitions_are_reused() {
        let mut scopes = ScopeInterner::default();
        let mut templates = ScopeTemplateInterner::default();
        let source = scopes.intern("source.test");
        let template = templates
            .intern_scope_template("meta.group keyword.control string.quoted", &mut scopes);
        let mut stacks = ScopeStackInterner::default();
        let root = stacks.push(stacks.empty(), source, &scopes);

        let pushed = stacks.push_template(root, template, &templates, &scopes);
        let node_count = stacks.len();
        assert_eq!(stacks.template_edges.get(&(root, template)), Some(&pushed));
        assert_eq!(
            stacks.push_template(root, template, &templates, &scopes),
            pushed
        );
        assert_eq!(stacks.len(), node_count);
        assert_eq!(stacks.template_edges.len(), 1);

        let pushed_once = stacks.push_template_once(root, template, &templates, &scopes);
        let node_count = stacks.len();
        assert_eq!(
            stacks.template_once_edges.get(&(root, template)),
            Some(&pushed_once)
        );
        assert_eq!(
            stacks.push_template_once(root, template, &templates, &scopes),
            pushed_once
        );
        assert_eq!(stacks.len(), node_count);
        assert_eq!(stacks.template_once_edges.len(), 1);
    }

    #[test]
    fn cached_template_pushes_preserve_resolve_and_class_behavior() {
        let mut scopes = ScopeInterner::default();
        let mut templates = ScopeTemplateInterner::default();
        let source = scopes.intern("source.test");
        let attribute = scopes.intern("entity.other.attribute-name.html");
        let tag = scopes.intern("entity.name.tag.html");
        let string = scopes.intern("string.quoted.double");
        let template = templates
            .intern_scope_template("entity.name.tag.html string.quoted.double", &mut scopes);

        let mut stacks = ScopeStackInterner::default();
        let root = stacks.push(stacks.empty(), source, &scopes);
        let root = stacks.push(root, attribute, &scopes);

        let mut expected = root;
        for scope in [tag, string] {
            expected = stacks.push(expected, scope, &scopes);
        }
        let pushed = stacks.push_template(root, template, &templates, &scopes);
        assert_eq!(pushed, expected);
        assert_eq!(
            stacks.resolve(pushed, &scopes),
            [
                "source.test",
                "entity.other.attribute-name.html",
                "entity.name.tag.html",
                "string.quoted.double",
            ]
        );
        assert_eq!(stacks.class(pushed), Some(SyntaxClass::Tag));
        assert_eq!(
            stacks.class(pushed),
            classify_scope_stack(&stacks.resolve(pushed, &scopes))
        );

        let tag_root = stacks.push(root, tag, &scopes);
        let mut expected_once = tag_root;
        for scope in [tag, string] {
            if stacks.top_scope(expected_once) != Some(scope) {
                expected_once = stacks.push(expected_once, scope, &scopes);
            }
        }
        let pushed_once = stacks.push_template_once(tag_root, template, &templates, &scopes);
        assert_eq!(pushed_once, expected_once);
        assert_eq!(
            stacks.resolve(pushed_once, &scopes),
            [
                "source.test",
                "entity.other.attribute-name.html",
                "entity.name.tag.html",
                "string.quoted.double",
            ]
        );
        assert_eq!(stacks.class(pushed_once), Some(SyntaxClass::Tag));
        assert_eq!(
            stacks.class(pushed_once),
            classify_scope_stack(&stacks.resolve(pushed_once, &scopes))
        );
    }
}
