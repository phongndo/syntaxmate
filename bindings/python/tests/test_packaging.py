from __future__ import annotations

from importlib import metadata

import pytest

# Both build variants embed the bundled themes; the default one also embeds the
# grammars. Their notices must travel inside the installed distribution.
NOTICES = [
    "third-party/themes/licenses/github-vscode-themes.license",
    "third-party/themes/licenses.json",
    "third-party/grammars/licenses.json",
    "third-party/grammars/licenses/rego.tmLanguage.license",
]


def test_distribution_ships_third_party_notices():
    try:
        dist = metadata.distribution("syntaxmate")
    except metadata.PackageNotFoundError:
        pytest.skip("syntaxmate is not installed as a distribution")
    shipped = {str(path).replace("\\", "/") for path in dist.files or ()}
    licenses = f"syntaxmate-{dist.version}.dist-info/licenses/"
    missing = [notice for notice in NOTICES if licenses + notice not in shipped]
    assert not missing, f"wheel lacks license notices: {missing}"
    theme = dist.locate_file(licenses + NOTICES[0]).read_text(encoding="utf-8")
    assert "Copyright (c) 2020 Primer" in theme
