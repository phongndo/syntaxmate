use std::io::{self, BufRead};

use serde::{Deserialize, Serialize};
use syntaxmate::diagnostics::{RegexAnchorContext, RegexEngine, match_regex};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplayInput {
    patterns: Vec<String>,
    line: String,
    from: usize,
    allow_start_of_file: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplayOutput {
    winner: Option<ReplayWinner>,
    errors: Vec<ReplayError>,
}

#[derive(Debug, Serialize)]
struct ReplayWinner {
    index: usize,
    captures: Vec<Option<ReplayRange>>,
}

#[derive(Debug, Serialize)]
struct ReplayRange {
    start: usize,
    end: usize,
}

#[derive(Debug, Serialize)]
struct ReplayError {
    index: usize,
    message: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    for (line_number, input) in io::stdin().lock().lines().enumerate() {
        let input = input.map_err(|error| error.to_string())?;
        if input.is_empty() {
            continue;
        }
        let input: ReplayInput = serde_json::from_str(&input)
            .map_err(|error| format!("input line {}: {error}", line_number + 1))?;
        let mut winner = None;
        let mut errors = Vec::new();
        for (index, pattern) in input.patterns.iter().enumerate() {
            match match_regex(
                pattern,
                &input.line,
                input.from,
                RegexAnchorContext {
                    allow_start_of_file: input.allow_start_of_file,
                    continuation_position: Some(input.from),
                },
                RegexEngine::Auto,
                10_000_000,
            ) {
                Ok(report) => {
                    let Some(range) = report.matched else {
                        continue;
                    };
                    let replace = match &winner {
                        None => true,
                        Some((winner_index, winner_start, _)) => {
                            range.start < *winner_start
                                || (range.start == *winner_start && index < *winner_index)
                        }
                    };
                    if replace {
                        winner = Some((index, range.start, report.captures));
                    }
                }
                Err(error) => errors.push(ReplayError {
                    index,
                    message: error.to_string(),
                }),
            }
        }
        let output = ReplayOutput {
            winner: winner.map(|(index, _, captures)| ReplayWinner {
                index,
                captures: captures
                    .into_iter()
                    .map(|range| {
                        range.map(|range| ReplayRange {
                            start: range.start,
                            end: range.end,
                        })
                    })
                    .collect(),
            }),
            errors,
        };
        println!(
            "{}",
            serde_json::to_string(&output).map_err(|error| error.to_string())?
        );
    }
    Ok(())
}
