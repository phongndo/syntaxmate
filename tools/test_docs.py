#!/usr/bin/env python3
"""Regression checks for documentation links and generated summaries."""

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def load_tool(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "tools" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


docs = load_tool("check-docs")
languages = load_tool("check-language-docs")


class DocumentationTests(unittest.TestCase):
    def test_fixture_markdown_is_not_guidance(self):
        self.assertFalse(docs.is_documentation(Path("tests/fixtures/textmate/markdown/stress.md")))
        self.assertFalse(docs.is_documentation(Path("benchmarks/textmate/corpora/sample.md")))
        self.assertTrue(docs.is_documentation(Path("tests/fixtures/textmate/README.md")))
        self.assertTrue(docs.is_documentation(Path("AGENTS.md")))
        self.assertTrue(docs.is_documentation(Path("docs/architecture.md")))

    def test_links_ignore_fenced_examples_and_accept_reference_definitions(self):
        text = '''[live](ok.md#heading)
```markdown
[example](missing.md)
```
~~~
[another example](also-missing.md)
~~~
[reference]: <space name.md> "title"
'''
        self.assertEqual(list(docs.link_targets(text)), ["ok.md#heading", "space name.md"])

    def test_local_links_resolve_relative_to_the_document(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "docs").mkdir()
            (root / "space name.md").write_text("# Example\n")
            guide = root / "docs/guide.md"
            guide.write_text(
                "[file](../space%20name.md#example) [directory](../docs/)\n"
                "[external](https://example.invalid/missing) [section](#section)\n"
            )
            self.assertEqual(docs.check_links(root, [Path("docs/guide.md")]), [])
            guide.write_text("[removed](removed.md)\n")
            self.assertIn("missing local link target removed.md", docs.check_links(root, [Path("docs/guide.md")])[0])

    def test_missing_package_document_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[package]\ninclude = ["/README.md", "src/**"]\n')
            self.assertEqual(len(docs.check_package_docs(root)), 1)
            (root / "README.md").write_text("# Readme\n")
            self.assertEqual(docs.check_package_docs(root), [])

    def test_generated_summaries_have_one_home_and_are_current(self):
        counts = languages.collect_counts(ROOT)
        policy = languages.load_scale_policy(ROOT, counts)
        rendered = languages.render_docs(ROOT, counts, policy)
        self.assertEqual(set(rendered), {languages.FIXTURE_README})
        for path, expected in rendered.items():
            self.assertEqual((ROOT / path).read_text(), expected)
        snippet = languages.count_snippet(languages.FIXTURE_README, counts)
        self.assertNotIn("The current validated IDs are", snippet)
        self.assertIn("../../../docs/language-status.md", snippet)


if __name__ == "__main__":
    unittest.main()
