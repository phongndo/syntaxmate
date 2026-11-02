use syntaxmate::Catalog;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut max_bytes = None;
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--assert-max-bundle-bytes" => {
                max_bytes = Some(
                    args.next()
                        .ok_or("--assert-max-bundle-bytes requires a value")?
                        .parse::<usize>()?,
                );
            }
            other => return Err(format!("unexpected argument {other:?}").into()),
        }
    }
    let summary = Catalog::bundled().bundle_summary();
    println!(
        "bundle={} bytes; languages={}; grammars={}; version={}",
        summary.bundle_bytes, summary.language_count, summary.grammar_count, summary.version
    );
    if let Some(max_bytes) = max_bytes
        && summary.bundle_bytes > max_bytes
    {
        return Err(format!(
            "bundle is {} bytes, exceeding {max_bytes}",
            summary.bundle_bytes
        )
        .into());
    }
    Ok(())
}
