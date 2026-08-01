use syntaxmate::diagnostics::{RegexAnchorContext, RegexEngine, inspect_regex, match_regex};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let match_mode = take_flag(&mut args, "--match");
    let engine = match take_value(&mut args, "--engine").as_deref() {
        None | Some("auto") => RegexEngine::Auto,
        Some("dfa" | "automata") => RegexEngine::Dfa,
        Some("fallback") => RegexEngine::Fallback,
        Some(engine) => return Err(format!("unknown --engine {engine:?}")),
    };
    let from = take_value(&mut args, "--from")
        .map(|value| value.parse::<usize>().map_err(|_| "--from must be a usize"))
        .transpose()?
        .unwrap_or(0);
    let budget = take_value(&mut args, "--budget")
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| "--budget must be a usize")
        })
        .transpose()?
        .unwrap_or(100_000);
    let allow_start_of_file = take_flag(&mut args, "--allow-a");
    let continuation_position = take_value(&mut args, "--allow-g")
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| "--allow-g must be a usize")
        })
        .transpose()?;
    let pattern = args.first().ok_or("missing pattern")?;

    let inspection = inspect_regex(pattern);
    println!("{}", inspection.parsed);
    println!("translated_pattern: {}", inspection.translated_pattern);
    println!("anchor_strategy: {}", inspection.anchor_strategy);
    println!("route: {}", inspection.route);
    if !match_mode {
        return Ok(());
    }

    let line = args.get(1).ok_or("--match needs a line argument")?;
    let report = match_regex(
        pattern,
        line,
        from,
        RegexAnchorContext {
            allow_start_of_file,
            continuation_position,
        },
        engine,
        budget,
    )
    .map_err(|error| error.to_string())?;
    println!("engine: {}", report.engine);
    if let Some(steps) = report.steps {
        println!("steps: {steps}");
    }
    match report.matched {
        Some(range) => println!("match: {}..{}", range.start, range.end),
        None => println!("match: <none>"),
    }
    for (index, capture) in report.captures.iter().enumerate() {
        println!("capture[{index}]: {capture:?}");
    }
    Ok(())
}

fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    if let Some(index) = args.iter().position(|argument| argument == flag) {
        args.remove(index);
        true
    } else {
        false
    }
}

fn take_value(args: &mut Vec<String>, flag: &str) -> Option<String> {
    let index = args.iter().position(|argument| argument == flag)?;
    args.remove(index);
    (index < args.len()).then(|| args.remove(index))
}
