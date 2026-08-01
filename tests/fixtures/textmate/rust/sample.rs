#![allow(dead_code)]

/// Golden fixture with unicode: λ🚀.
pub fn greet<'a>(name: &'a str) -> String {
    let raw = r#"hello \"textmate\" \n"#;
    let escaped = "tab\tunicode λ and quote \"";
    format!("{name}: {raw}::{escaped}")
}

/* outer comment
   /* nested comment */
   done
*/
macro_rules! mark_fixture { ($value:expr) => { Some($value) }; }

#[cfg(test)]
mod tests {
    #[test]
    fn sample() {
        assert_eq!(super::greet("mark").contains("mark"), true);
    }
}
