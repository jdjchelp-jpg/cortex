//! Text / JSON flashcard import parsers. Powers the "Import Flashcards" flow's
//! format picker (Anki JSON, Quizlet, Memrise, and the plain-text variants).
//!
//! Every parser is pure — `&str` in, `Vec<(front, back)>` out — so the whole
//! module is exhaustively unit-tested without a DB or Tauri. The command layer
//! (`commands::import_flashcards`) layers dedupe + persistence on top.

use crate::error::{Error, Result};
use serde_json::Value;

/// Front-side keys we recognise inside a JSON card object (in priority order).
const FRONT_KEYS: &[&str] = &["front", "q", "question", "term", "word", "prompt", "side1"];
/// Back-side keys we recognise inside a JSON card object (in priority order).
const BACK_KEYS: &[&str] = &["back", "a", "answer", "definition", "translation", "response", "side2"];

/// Parse pasted flashcard content into `(front, back)` pairs for the given
/// `format` id. `format` is a stable id emitted by the UI's format picker; an
/// unknown id is a hard error so a UI/backend mismatch fails loudly instead of
/// silently importing nothing.
pub fn parse(format: &str, text: &str) -> Result<Vec<(String, String)>> {
    let cards = match format {
        // JSON accepts front/back, q/a, term/definition, word/translation, …
        "anki_json" | "json" => parse_json(text)?,
        // Quizlet / Memrise exports are two-column: tab-preferred, else blocks.
        "quizlet" | "memrise" => parse_two_col(text),
        // CSV (RFC 4180): one card per row, column 1 = front, column 2 = back.
        "csv" => parse_csv(text),
        // "Term:" / "Definition:" blocks separated by blank lines.
        "newline" | "termdef" => parse_labeled(text, "term", "definition"),
        // One "front<sep>back" per line, split on the first separator.
        "tab" => parse_sep(text, "\t"),
        "colon" => parse_sep(text, ":"),
        // "Q:" / "A:" blocks.
        "qa" => parse_labeled(text, "q", "a"),
        other => return Err(Error::Other(format!("unknown import format: {other}"))),
    };
    Ok(cards)
}

/// Split text into blocks of consecutive non-blank lines (blank line = boundary).
fn split_blocks(text: &str) -> Vec<Vec<String>> {
    let mut blocks: Vec<Vec<String>> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    for raw in text.lines() {
        let l = raw.trim();
        if l.is_empty() {
            if !cur.is_empty() {
                blocks.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(l.to_string());
        }
    }
    if !cur.is_empty() {
        blocks.push(cur);
    }
    blocks
}

/// Flush a pending labeled card (front+back both present and non-empty).
fn flush_labeled(out: &mut Vec<(String, String)>, front: &mut Option<String>, back: &mut Option<String>) {
    if let (Some(f), Some(b)) = (front.take(), back.take()) {
        if !f.trim().is_empty() && !b.trim().is_empty() {
            out.push((f, b));
        }
    }
}
/// Parse "Label: value" blocks — e.g. Term/Definition or Q/A. Robust to both
/// blank-line-separated and back-to-back cards; a labelled front starts a new
/// card (flushing the previous), and continuation lines extend the current back.
fn parse_labeled(text: &str, front_label: &str, back_label: &str) -> Vec<(String, String)> {
    let fl = format!("{front_label}:");
    let bl = format!("{back_label}:");
    let mut out: Vec<(String, String)> = Vec::new();
    let mut cur_front: Option<String> = None;
    let mut cur_back: Option<String> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            flush_labeled(&mut out, &mut cur_front, &mut cur_back);
            continue;
        }
        let lower = line.to_lowercase();
        if lower.starts_with(&fl) {
            flush_labeled(&mut out, &mut cur_front, &mut cur_back);
            cur_front = Some(line[fl.len()..].trim().to_string());
        } else if lower.starts_with(&bl) {
            let val = line[bl.len()..].trim();
            match &mut cur_back {
                Some(b) => {
                    b.push(' ');
                    b.push_str(val);
                }
                None => cur_back = Some(val.to_string()),
            }
        } else if let (Some(_), Some(b)) = (cur_front.as_ref(), cur_back.as_mut()) {
            // Unlabeled continuation of the answer.
            b.push(' ');
            b.push_str(line);
        }
        // Stray line before any front label: ignore.
    }
    flush_labeled(&mut out, &mut cur_front, &mut cur_back);
    out
}

/// One card per non-blank line: `front<sep>back`, split on the FIRST separator.
/// A line with no separator (or an empty front) is skipped.
fn parse_sep(text: &str, sep: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(idx) = line.find(sep) {
            let front = line[..idx].trim().to_string();
            let back = line[idx + sep.len()..].trim().to_string();
            if !front.is_empty() {
                out.push((front, back));
            }
        }
    }
    out
}

/// Two-column paste (Quizlet / Memrise). Prefer tab separation (their native
/// export); otherwise treat each blank-line-separated block as front = first
/// line, back = remaining lines joined.
fn parse_two_col(text: &str) -> Vec<(String, String)> {
    if text.contains('\t') {
        return parse_sep(text, "\t");
    }
    let mut out = Vec::new();
    for block in split_blocks(text) {
        if block.len() >= 2 {
            out.push((block[0].clone(), block[1..].join(" ")));
        }
    }
    out
}

/// First non-empty string among `keys` in a JSON object.
fn json_str(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(s) = obj.get(*k).and_then(|x| x.as_str()) {
            let s = s.trim();
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// Recursively collect cards from any JSON shape: an array of `[front, back]`
/// pairs or card objects, or a container object wrapping a `cards`/`notes`/…
/// array (so a whole Anki-style export can be pasted whole).
fn collect_json_cards(v: &Value, out: &mut Vec<(String, String)>) {
    match v {
        Value::Array(arr) => {
            for item in arr {
                // [front, back] pair.
                if let Some(pair) = item.as_array() {
                    if pair.len() >= 2 {
                        if let (Some(f), Some(b)) = (pair[0].as_str(), pair[1].as_str()) {
                            out.push((f.trim().to_string(), b.trim().to_string()));
                            continue;
                        }
                    }
                }
                if let Some(obj) = item.as_object() {
                    // Anki note shape: { "fields": ["front", "back"] }.
                    if let Some(fields) = obj.get("fields").and_then(|x| x.as_array()) {
                        if fields.len() >= 2 {
                            if let (Some(f), Some(b)) = (fields[0].as_str(), fields[1].as_str()) {
                                out.push((f.trim().to_string(), b.trim().to_string()));
                                continue;
                            }
                        }
                    }
                    if let (Some(f), Some(b)) = (json_str(obj, FRONT_KEYS), json_str(obj, BACK_KEYS)) {
                        out.push((f, b));
                    }
                }
            }
        }
        Value::Object(map) => {
            for key in ["cards", "flashcards", "notes", "deck", "items", "data"] {
                if let Some(inner) = map.get(key) {
                    collect_json_cards(inner, out);
                    if !out.is_empty() {
                        return;
                    }
                }
            }
            if let (Some(f), Some(b)) = (json_str(map, FRONT_KEYS), json_str(map, BACK_KEYS)) {
                out.push((f, b));
            }
        }
        _ => {}
    }
}

fn parse_json(text: &str) -> Result<Vec<(String, String)>> {
    let v: Value = serde_json::from_str(text.trim())
        .map_err(|e| Error::Other(format!("invalid JSON: {e}")))?;
    let mut out = Vec::new();
    collect_json_cards(&v, &mut out);
    Ok(out)
}

/// Parse CSV flashcards (RFC 4180): one card per data row, column 1 = front,
/// column 2 = back. Handles quoted fields with embedded commas/newlines and
/// `""` escaped quotes, so a term like `"…20 feet away?"` (which contains a
/// comma) stays intact. A leading header row whose two cells are recognised
/// column labels (front/back, term/definition, …) is skipped. Extra columns are
/// ignored; a row with an empty front is skipped.
fn parse_csv(text: &str) -> Vec<(String, String)> {
    let rows = csv_rows(text);
    let mut out: Vec<(String, String)> = Vec::new();
    for (i, row) in rows.into_iter().enumerate() {
        if row.len() < 2 {
            continue;
        }
        let front = row[0].trim().to_string();
        let back = row[1].trim().to_string();
        if front.is_empty() {
            continue;
        }
        // Skip a header row like `front,back` / `term,definition` / `q,a`.
        if i == 0 && is_csv_header(&front, &back) {
            continue;
        }
        out.push((front, back));
    }
    out
}

/// True when a CSV's first row looks like column labels rather than a card.
fn is_csv_header(front: &str, back: &str) -> bool {
    let f = front.to_lowercase();
    let b = back.to_lowercase();
    let front_ok = matches!(f.as_str(), "front" | "term" | "question" | "q" | "word" | "prompt");
    let back_ok = matches!(b.as_str(), "back" | "definition" | "answer" | "a" | "translation" | "response");
    front_ok && back_ok
}

/// Split CSV text into rows of string fields per RFC 4180: fields are
/// comma-separated, a field may be wrapped in double quotes (inside which `""`
/// is a literal quote and commas/newlines are literal), and rows end at a
/// newline. Handles both `\n` and `\r\n` line endings.
fn csv_rows(text: &str) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut field_started = false; // any char seen for the current field (incl. quotes)
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
        } else {
            match c {
                '"' => {
                    in_quotes = true;
                    field_started = true;
                }
                ',' => {
                    row.push(std::mem::take(&mut field));
                    field_started = false;
                }
                '\r' => { /* handle CRLF and lone CR as a row break */ }
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                    field_started = false;
                }
                _ => {
                    field.push(c);
                    field_started = true;
                }
            }
        }
    }
    // Flush the final field/row if there's anything pending (no trailing newline).
    if field_started || !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn termdef_newline_blocks() {
        let text = "Term: What is a computer cable?\nDefinition: A cable transmits power or data.\n\nTerm: Two main types?\nDefinition: Data and power.";
        let c = parse("newline", text).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].0, "What is a computer cable?");
        assert_eq!(c[0].1, "A cable transmits power or data.");
        assert_eq!(c[1].1, "Data and power.");
    }

    #[test]
    fn termdef_back_to_back_no_blank_lines() {
        let text = "Term: A\nDefinition: aa\nTerm: B\nDefinition: bb";
        let c = parse("termdef", text).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[1], ("B".to_string(), "bb".to_string()));
    }

    #[test]
    fn qa_blocks() {
        let text = "Q: 2+2?\nA: 4\n\nQ: Capital of France?\nA: Paris";
        let c = parse("qa", text).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0], ("2+2?".to_string(), "4".to_string()));
        assert_eq!(c[1].1, "Paris");
    }

    #[test]
    fn tab_separated() {
        let text = "hola\thello\nadios\tgoodbye";
        let c = parse("tab", text).unwrap();
        assert_eq!(c, vec![("hola".to_string(), "hello".to_string()), ("adios".to_string(), "goodbye".to_string())]);
    }

    #[test]
    fn quizlet_prefers_tab() {
        let text = "RAM\tRandom Access Memory\nROM\tRead-Only Memory";
        let c = parse("quizlet", text).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].1, "Random Access Memory");
    }

    #[test]
    fn memrise_block_fallback() {
        let text = "hola\nhello\n\nadios\ngoodbye";
        let c = parse("memrise", text).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0], ("hola".to_string(), "hello".to_string()));
        assert_eq!(c[1], ("adios".to_string(), "goodbye".to_string()));
    }

    #[test]
    fn colon_separated() {
        let text = "What is RAM?: Random Access Memory.\nSpeed of light?: ~3e8 m/s";
        let c = parse("colon", text).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].1, "Random Access Memory.");
    }

    #[test]
    fn json_front_back_array() {
        let text = r#"[{"front":"Q1","back":"A1"},{"front":"Q2","back":"A2"}]"#;
        let c = parse("anki_json", text).unwrap();
        assert_eq!(c, vec![("Q1".to_string(), "A1".to_string()), ("Q2".to_string(), "A2".to_string())]);
    }

    #[test]
    fn json_q_a_and_term_def_mixed() {
        let text = r#"[{"q":"2+2","a":"4"},{"term":"hola","definition":"hello"}]"#;
        let c = parse("json", text).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[1], ("hola".to_string(), "hello".to_string()));
    }

    #[test]
    fn json_pair_arrays_and_fields() {
        let text = r#"[["f1","b1"],{"fields":["f2","b2"]}]"#;
        let c = parse("anki_json", text).unwrap();
        assert_eq!(c, vec![("f1".to_string(), "b1".to_string()), ("f2".to_string(), "b2".to_string())]);
    }

    #[test]
    fn json_wrapped_container() {
        let text = r#"{"deck":"X","cards":[{"word":"libro","translation":"book"}]}"#;
        let c = parse("anki_json", text).unwrap();
        assert_eq!(c, vec![("libro".to_string(), "book".to_string())]);
    }

    #[test]
    fn json_invalid_errors() {
        assert!(parse("anki_json", "not json").is_err());
    }

    #[test]
    fn unknown_format_errors() {
        assert!(parse("nope", "whatever").is_err());
    }

    #[test]
    fn empty_and_blank_inputs_yield_none() {
        assert!(parse("tab", "").unwrap().is_empty());
        assert!(parse("newline", "   \n\n").unwrap().is_empty());
        assert!(parse("qa", "Q: only front\n").unwrap().is_empty());
    }

    #[test]
    fn csv_basic_two_columns() {
        let text = "What is RAM?,Random Access Memory.\nWhat is ROM?,Read-Only Memory.";
        let c = parse("csv", text).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0], ("What is RAM?".to_string(), "Random Access Memory.".to_string()));
        assert_eq!(c[1].1, "Read-Only Memory.");
    }

    #[test]
    fn csv_quoted_field_with_comma() {
        // The term contains a comma, so it must be quoted — a naive comma split
        // would break this into the wrong number of columns.
        let text = "\"According to the 20-20-20 Rule, how long should you look at something 20 feet away?\",At least 20 seconds.";
        let c = parse("csv", text).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].0, "According to the 20-20-20 Rule, how long should you look at something 20 feet away?");
        assert_eq!(c[0].1, "At least 20 seconds.");
    }

    #[test]
    fn csv_escaped_quotes_and_crlf() {
        let text = "\"He said \"\"hi\"\"\",greeting\r\nplain,simple";
        let c = parse("csv", text).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].0, "He said \"hi\"");
        assert_eq!(c[0].1, "greeting");
        assert_eq!(c[1], ("plain".to_string(), "simple".to_string()));
    }

    #[test]
    fn csv_skips_header_row() {
        let text = "front,back\nQ1,A1\nQ2,A2";
        let c = parse("csv", text).unwrap();
        assert_eq!(c, vec![("Q1".to_string(), "A1".to_string()), ("Q2".to_string(), "A2".to_string())]);
    }

    #[test]
    fn csv_ignores_extra_columns_and_empty_fronts() {
        let text = "Q1,A1,extra,more\n,A2\nQ2,A2";
        let c = parse("csv", text).unwrap();
        assert_eq!(c, vec![("Q1".to_string(), "A1".to_string()), ("Q2".to_string(), "A2".to_string())]);
    }
}

