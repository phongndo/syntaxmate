#![allow(dead_code)]

/// One borrowed sensor label, including café, rockets 🚀, and 𝌆.
#[derive(Debug, Clone, PartialEq)]
struct Reading<'a> {
    label: &'a str,
    value: f64,
}

fn summarize(readings: &[Reading<'_>]) -> Result<String, &'static str> {
    let first = readings.first().ok_or("no readings")?;
    let peak = readings.iter().map(|item| item.value).reduce(f64::max).unwrap();
    let raw = r#"units: "km/s" 🚀 𝌆"#;
    Ok(format!("{:?}: peak {peak:.1} {raw}", first.label))
}

fn main() {
    let readings = [Reading {
        label: "café",
        value: 0x2A as f64,
    }];
    match summarize(&readings) {
        Ok(message) => println!("{message}"),
        Err(error) => eprintln!("error: {error}"),
    }
}
