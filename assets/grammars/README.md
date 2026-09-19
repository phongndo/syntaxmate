# Grammar sources

These are committed, reviewable TextMate JSON inputs. The active public IDs and
private dependency grammars are defined in [coverage.toml](coverage.toml).
[language-metadata.json](language-metadata.json) supplies detection metadata;
[coverage.full-shiki.toml](coverage.full-shiki.toml) is the generated Shiki import
baseline, not the complete public catalog.

[SOURCE.toml](SOURCE.toml) records immutable upstream pins, overrides, and
transformations. [licenses.json](licenses.json) references per-asset notices in
[licenses/](licenses/). Preserve intentionally unresolved optional includes:
loading an extra grammar can change behavior relative to the upstream host.

The runtime artifact is the committed [grammars.bundle](../grammars.bundle),
generated ahead of release by [build-bundle.rs](../../tools/build-bundle.rs).
Normal Cargo builds do not generate it or run Node.

Follow [asset maintenance](../../docs/assets.md) to update sources, licenses,
detection, the bundle, and oracle output together.
