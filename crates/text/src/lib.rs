//! Excerpt-only markup reduction; the original source remains authoritative.
/// Strip inline display markup, including nested constructs and malformed delimiters.
pub fn plain_text(markup: &str) -> String {
    let mut stack: Vec<(char, String)> = Vec::new();
    let mut output = String::new();
    let mut chars = markup.chars().peekable();
    while let Some(ch) = chars.next() {
        let text = match ch {
            '《' | '【' => {
                stack.push((ch, String::new()));
                continue;
            }
            '》' | '】' => {
                let opening = if ch == '》' { '《' } else { '【' };
                if stack.last().is_some_and(|(kind, _)| *kind == opening) {
                    if let Some((kind, body)) = stack.pop() {
                        reduce(kind, &body)
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            }
            '※' => {
                while chars.peek().is_some_and(|ch| *ch != '\n' && *ch != '\r') {
                    chars.next();
                }
                continue;
            }
            '＃' => {
                let mut removed = false;
                while chars
                    .peek()
                    .is_some_and(|ch| ch.is_ascii_digit() || ('０'..='９').contains(ch))
                {
                    chars.next();
                    removed = true;
                }
                if removed {
                    continue;
                }
                ch.to_string()
            }
            _ => ch.to_string(),
        };
        if let Some((_, body)) = stack.last_mut() {
            body.push_str(&text);
        } else {
            output.push_str(&text);
        }
    }
    while let Some((kind, body)) = stack.pop() {
        let text = reduce(kind, &body);
        if let Some((_, parent)) = stack.last_mut() {
            parent.push_str(&text);
        } else {
            output.push_str(&text);
        }
    }
    output
}
fn reduce(kind: char, body: &str) -> String {
    if kind == '【' {
        return if body == "右丁" || body == "左丁" {
            "\n".into()
        } else {
            String::new()
        };
    }
    let Some((name, text)) = body.split_once('：') else {
        return body.into();
    };
    match name {
        "振り仮名" | "圏点" => text.split('｜').next().unwrap_or_default().into(),
        "見せ消ち" => text
            .split_once('｜')
            .map(|(_, after)| after)
            .unwrap_or_default()
            .into(),
        _ => text.into(),
    }
}
/// Limit to Unicode scalar values, then append an ellipsis only when truncated.
pub fn excerpt(markup: &str, max_chars: usize) -> String {
    let plain = plain_text(markup);
    let mut chars = plain.chars();
    let mut result: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        result.push('…');
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    #[test]
    fn syntax_and_unicode() {
        for (source, want) in [
            ("《割書：a｜b》", "a｜b"),
            ("《振り仮名：base｜ruby》", "base"),
            ("《見せ消ち：a｜b》", "b"),
            ("《圏点：x｜m》", "x"),
            ("《右線：x》", "x"),
            ("【注】a【右丁】b【左丁】", "a\nb\n"),
            ("字※注釈\n文＃１０■□〓＃2", "字\n文■□〓"),
            ("《割書：a｜《圏点：b｜﹅》》", "a｜b"),
        ] {
            assert_eq!(plain_text(source), want);
        }
        assert_eq!(excerpt("𛀁日本", 2), "𛀁日…");
        assert_eq!(excerpt("𛀁日本", 3), "𛀁日本");
        assert_eq!(excerpt("字", 0), "…");
    }
    #[test]
    fn entry_samples() -> Result<(), serde_json::Error> {
        let entry: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/api/entry-0916dafb80cdc48ca7687afcad4a4f35.json"
        ))?;
        let pages = entry["transcriptions"].as_array().ok_or_else(|| {
            <serde_json::Error as serde::de::Error>::custom("missing transcriptions")
        })?;
        assert_eq!(
            plain_text(pages[0]["text"].as_str().unwrap_or_default()),
            "蝦夷紀行　上"
        );
        for page in pages {
            let text = page["text"].as_str().unwrap_or_default();
            assert!(!plain_text(text).contains(['《', '》', '【', '】']));
            assert!(excerpt(text, 30).chars().count() <= 31);
        }
        Ok(())
    }
    proptest! {
        #[test]
        fn no_markup_delimiters(chars in proptest::collection::vec(any::<char>(),0..500)) {
            let source: String = chars.into_iter().collect();
            prop_assert!(!plain_text(&source).contains(['《','》','【','】']));
        }
        #[test]
        fn delimiter_heavy(source in "[《》【】割書：｜a※＃０-９右左丁\\n]{0,300}") {
            prop_assert!(!plain_text(&source).contains(['《','》','【','】']));
        }
    }
}
