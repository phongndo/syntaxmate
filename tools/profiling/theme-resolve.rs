use std::io::{self, BufRead};

use serde_json::json;
use syntaxmate::{FontModifiers, RgbColor, Theme};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let name = std::env::args()
        .nth(1)
        .ok_or("usage: theme-resolve THEME_OR_JSON_PATH")?;
    let theme = match Theme::bundled(&name) {
        Ok(theme) => theme,
        Err(_) => {
            let source = std::fs::read_to_string(&name)?;
            Theme::from_json(&source)?
        }
    };

    for line in io::stdin().lock().lines() {
        let line = line?;
        let scopes: Vec<String> = serde_json::from_str(&line)?;
        let names = scopes.iter().map(String::as_str).collect::<Vec<_>>();
        let style = theme.resolve_scope_names(&names);
        let color = |color: Option<RgbColor>| {
            color.map(|color| format!("#{:02x}{:02x}{:02x}", color.red, color.green, color.blue))
        };
        let mut modifiers = Vec::new();
        for (modifier, name) in [
            (FontModifiers::ITALIC, "italic"),
            (FontModifiers::BOLD, "bold"),
            (FontModifiers::UNDERLINED, "underline"),
            (FontModifiers::CROSSED_OUT, "strikethrough"),
        ] {
            if style.modifiers.contains(modifier) {
                modifiers.push(name);
            }
        }
        println!(
            "{}",
            json!({
                "foreground": color(style.foreground),
                "background": color(style.background),
                "modifiers": modifiers,
            })
        );
    }
    Ok(())
}
