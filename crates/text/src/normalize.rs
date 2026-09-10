//! Search normalization in Unicode scalar coordinates.
use std::{collections::HashMap, sync::OnceLock};
use unicode_normalization::UnicodeNormalization;

/// Separates physical lines. Tokenizers and queries must never match it.
pub const SEPARATOR: char = '\0';
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Normalized {
    pub text: String,
    pub offsets: Vec<Span>,
}
type Mapped = Vec<(char, Span)>;

pub(crate) fn stripped(source: &str, half_breaks: bool) -> Mapped {
    let mut stack: Vec<(char, Mapped)> = Vec::new();
    let mut output = Vec::new();
    let mut chars = source.chars().enumerate().peekable();
    while let Some((index, ch)) = chars.next() {
        let span = Span {
            start: index as u32,
            end: index as u32 + 1,
        };
        let text = match ch {
            '《' | '【' => {
                stack.push((ch, Vec::new()));
                continue;
            }
            '》' | '】' => {
                let opening = if ch == '》' { '《' } else { '【' };
                if stack.last().is_some_and(|(kind, _)| *kind == opening) {
                    stack
                        .pop()
                        .map(|(kind, body)| reduce(kind, body, half_breaks))
                        .unwrap_or_default()
                } else {
                    Vec::new()
                }
            }
            '※' => {
                while chars
                    .peek()
                    .is_some_and(|(_, ch)| *ch != '\n' && *ch != '\r')
                {
                    chars.next();
                }
                continue;
            }
            '＃' if chars
                .peek()
                .is_some_and(|(_, ch)| ch.is_ascii_digit() || ('０'..='９').contains(ch)) =>
            {
                while chars
                    .peek()
                    .is_some_and(|(_, ch)| ch.is_ascii_digit() || ('０'..='９').contains(ch))
                {
                    chars.next();
                }
                continue;
            }
            _ => vec![(ch, span)],
        };
        if let Some((_, body)) = stack.last_mut() {
            body.extend(text);
        } else {
            output.extend(text);
        }
    }
    while let Some((kind, body)) = stack.pop() {
        let text = reduce(kind, body, half_breaks);
        if let Some((_, parent)) = stack.last_mut() {
            parent.extend(text);
        } else {
            output.extend(text);
        }
    }
    output
}
fn reduce(kind: char, body: Mapped, half_breaks: bool) -> Mapped {
    if kind == '【' {
        let name: String = body.iter().map(|(ch, _)| ch).collect();
        return if half_breaks && (name == "右丁" || name == "左丁") {
            body.first()
                .map(|(_, span)| vec![('\n', *span)])
                .unwrap_or_default()
        } else {
            Vec::new()
        };
    }
    let Some(colon) = body.iter().position(|(ch, _)| *ch == '：') else {
        return body;
    };
    let name: String = body[..colon].iter().map(|(ch, _)| ch).collect();
    let text = &body[colon + 1..];
    let pipe = text.iter().position(|(ch, _)| *ch == '｜');
    match name.as_str() {
        "振り仮名" | "圏点" => text[..pipe.unwrap_or(text.len())].to_vec(),
        "見せ消ち" => pipe.map(|p| text[p + 1..].to_vec()).unwrap_or_default(),
        "割書" => text.iter().copied().filter(|(ch, _)| *ch != '｜').collect(),
        _ => text.to_vec(),
    }
}
pub fn strict(source: &str) -> Normalized {
    let mut visible = stripped(source, false).into_iter().peekable();
    collect(source.chars().enumerate().filter_map(|(index, ch)| {
        let mapped = if visible
            .peek()
            .is_some_and(|(_, span)| span.start as usize == index)
        {
            visible.next()
        } else {
            None
        };
        if matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
            Some((
                SEPARATOR,
                Span {
                    start: index as u32,
                    end: index as u32 + 1,
                },
            ))
        } else {
            mapped.filter(|(ch, _)| !ch.is_whitespace())
        }
    }))
}
fn collect(chars: impl IntoIterator<Item = (char, Span)>) -> Normalized {
    let mut result = Normalized::default();
    for (ch, span) in chars {
        result.text.push(ch);
        result.offsets.push(span);
    }
    result
}
fn kana(ch: char) -> String {
    if ('\u{ff66}'..='\u{ff9f}').contains(&ch) {
        return ch
            .to_string()
            .nfkd()
            .flat_map(|c| kana(c).chars().collect::<Vec<_>>())
            .collect();
    }
    let ch = match ch {
        'ァ'..='ヶ' | 'ヽ' | 'ヾ' => char::from_u32(ch as u32 - 0x60).unwrap_or(ch),
        '𛀀' => 'え',
        '𛄠' => 'い',
        '𛄡' => '𛀁',
        '𛄢' => '𛄟',
        'ㇰ' => 'く',
        'ㇱ' => 'し',
        'ㇲ' => 'す',
        'ㇳ' => 'と',
        'ㇴ' => 'ぬ',
        'ㇵ' => 'は',
        'ㇶ' => 'ひ',
        'ㇷ' => 'ふ',
        'ㇸ' => 'へ',
        'ㇹ' => 'ほ',
        'ㇺ' => 'む',
        'ㇻ' => 'ら',
        'ㇼ' => 'り',
        'ㇽ' => 'る',
        'ㇾ' => 'れ',
        'ㇿ' => 'ろ',
        '𛅐' | '𛅤' => 'ゐ',
        '𛅑' | '𛅥' => 'ゑ',
        '𛅒' | '𛅦' => 'を',
        '𛅧' => 'ん',
        '𛄲' | '𛅕' => 'こ',
        _ => ch,
    };
    if matches!(ch, '\u{3099}' | '\u{309a}' | '゛' | '゜') {
        return String::new();
    }
    let small = |c| match c {
        'ぁ' => 'あ',
        'ぃ' => 'い',
        'ぅ' => 'う',
        'ぇ' => 'え',
        'ぉ' => 'お',
        'っ' => 'つ',
        'ゃ' => 'や',
        'ゅ' => 'ゆ',
        'ょ' => 'よ',
        'ゎ' => 'わ',
        'ゕ' => 'か',
        'ゖ' => 'け',
        _ => c,
    };
    if ('ぁ'..='ゖ').contains(&ch) {
        ch.to_string()
            .nfd()
            .filter(|c| !matches!(c, '\u{3099}' | '\u{309a}'))
            .map(small)
            .collect()
    } else if ('ヷ'..='ヺ').contains(&ch) {
        ch.to_string()
            .nfd()
            .filter(|c| *c != '\u{3099}')
            .flat_map(|c| kana(c).chars().collect::<Vec<_>>())
            .collect()
    } else {
        small(ch).to_string()
    }
}
fn is_kana(ch: char) -> bool {
    matches!(ch, 'ぁ'..='ゖ' | '\u{1b001}'..='\u{1b11f}' | '\u{1b150}'..='\u{1b152}')
}
fn is_kanji(ch: char) -> bool {
    matches!(ch, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{323af}')
}
fn kana_and_iteration(input: &Normalized) -> Mapped {
    let chars: Vec<_> = input
        .text
        .chars()
        .zip(input.offsets.iter().copied())
        .collect();
    let mut output: Mapped = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (ch, mut span) = chars[i];
        if matches!(ch, '〳' | '〴') && chars.get(i + 1).is_some_and(|(c, _)| *c == '〵') {
            span.end = chars[i + 1].1.end;
            if output.len() >= 2
                && output[output.len() - 2..]
                    .iter()
                    .all(|(c, _)| *c != SEPARATOR)
            {
                let pair = [output[output.len() - 2].0, output[output.len() - 1].0];
                output.extend(pair.map(|c| (c, span)));
            } else {
                output.extend([(ch, span), ('〵', span)]);
            }
            i += 2;
            continue;
        }
        let repeat = match ch {
            'ゝ' | 'ゞ' | 'ヽ' | 'ヾ' => {
                output.last().filter(|(c, _)| is_kana(*c)).map(|(c, _)| *c)
            }
            '々' | '〻' => output.last().filter(|(c, _)| is_kanji(*c)).map(|(c, _)| *c),
            _ => None,
        };
        if let Some(c) = repeat {
            output.push((c, span));
        } else {
            let folded = kana(ch);
            if folded.is_empty() {
                if let Some((c, previous)) = output.last_mut()
                    && *c != SEPARATOR
                {
                    previous.end = span.end;
                }
            } else {
                output.extend(folded.chars().map(|c| (c, span)));
            }
        }
        i += 1;
    }
    output
}
type VariantChoices = HashMap<char, Vec<(Vec<char>, Vec<char>)>>;
struct Variants {
    by_first: VariantChoices,
}
fn variants() -> &'static Variants {
    static TABLE: OnceLock<Variants> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut groups: Vec<Vec<String>> = Vec::new();
        for line in include_str!("../data/variants.txt")
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
        {
            let mut class: Vec<String> = line
                .split_whitespace()
                .map(|s| {
                    s.chars()
                        .flat_map(|c| kana(c).chars().collect::<Vec<_>>())
                        .collect()
                })
                .collect();
            if class.is_empty() {
                continue;
            }
            let mut first = groups.len();
            for i in (0..groups.len()).rev() {
                if groups[i].iter().any(|word| class.contains(word)) {
                    first = i;
                    let mut old = groups.remove(i);
                    old.extend(class);
                    class = old;
                }
            }
            groups.insert(first.min(groups.len()), class);
        }
        let mut by_first: VariantChoices = HashMap::new();
        for class in groups {
            let representative: Vec<char> = class[0].chars().collect();
            for word in class {
                let chars: Vec<_> = word.chars().collect();
                if let Some(first) = chars.first() {
                    by_first
                        .entry(*first)
                        .or_default()
                        .push((chars, representative.clone()));
                }
            }
        }
        for choices in by_first.values_mut() {
            choices.sort_by_key(|(word, _)| std::cmp::Reverse(word.len()));
            choices.dedup();
        }
        Variants { by_first }
    })
}
pub fn folded(source: &str) -> Normalized {
    fold(&strict(source))
}
pub fn fold(input: &Normalized) -> Normalized {
    let chars = kana_and_iteration(input);
    let mut output = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let choice = variants().by_first.get(&chars[i].0).and_then(|choices| {
            choices.iter().find(|(word, _)| {
                i + word.len() <= chars.len()
                    && word.iter().zip(&chars[i..]).all(|(a, (b, _))| a == b)
            })
        });
        if let Some((word, replacement)) = choice {
            let span = Span {
                start: chars[i].1.start,
                end: chars[i + word.len() - 1].1.end,
            };
            output.extend(replacement.iter().map(|c| (*c, span)));
            i += word.len();
        } else {
            output.push(chars[i]);
            i += 1;
        }
    }
    collect(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    #[test]
    fn rules_and_spans() {
        for (source, want) in [
            ("《振り仮名：base｜ruby》", "base"),
            ("《割書：a｜b》", "ab"),
            ("《見せ消ち：a｜b》", "b"),
            ("【注】 a　b※注\n＃１２c＃2", "ab\0c"),
            ("前【注\n釈】後", "前\0後"),
            ("《振り仮名：山｜や\nま》川", "山\0川"),
        ] {
            assert_eq!(strict(source).text, want);
        }
        for (source, want) in [
            ("ガパァッャㇷ", "かはあつやふ"),
            ("ｶﾞﾊﾟｧｯｬ", "かはあつや"),
            ("𛄡𛄢", "𛀁𛄟"),
            ("か\u{3099}は\u{309a}", "かは"),
            ("かゝかゞカヽカヾ", "かかかかかかかか"),
            ("山々山〻", "山山山山"),
            ("山川〳〵山川〴〵", "山川山川山川山川"),
            ("か\nゝ山\n々\n〳〵", "か\0ゝ山\0々\0〳〵"),
            ("之の", "のの"),
            ("也なり", "なりなり"),
            ("可べしベシ", "へしへしへし"),
            ("時时とき", "ときときとき"),
            ("総總惣", "総総総"),
            ("体體躰", "体体体"),
            ("國国讀読", "国国読読"),
            ("神神", "神神"),
        ] {
            assert_eq!(folded(source).text, want, "{source}");
        }
        let source = "【注】𛀁 葛\u{e0100}《振り仮名：𛀀｜え》\n也";
        let original: Vec<_> = source.chars().collect();
        let strict = strict(source);
        for (c, span) in strict.text.chars().zip(&strict.offsets) {
            assert_eq!(
                if c == SEPARATOR { '\n' } else { c },
                original[span.start as usize]
            );
            assert_eq!(span.end, span.start + 1);
        }
        let fold = folded(source);
        let tail = &fold.offsets[fold.offsets.len() - 2..];
        assert_eq!(tail[0], tail[1]);
        assert_eq!(original[tail[0].start as usize], '也');
    }
    proptest! {
        #[test]
        fn strict_offsets_reference_original(source in any::<String>()) {
            let chars: Vec<_> = source.chars().collect();
            let normalized = strict(&source);
            prop_assert_eq!(normalized.text.chars().count(), normalized.offsets.len());
            for (ch, span) in normalized.text.chars().zip(normalized.offsets) {
                prop_assert!((span.start as usize) < chars.len());
                prop_assert_eq!(span.end, span.start+1);
                prop_assert!(ch == chars[span.start as usize] || (ch == SEPARATOR && matches!(chars[span.start as usize], '\r' | '\n' | '\u{2028}' | '\u{2029}')), "mapped scalar");
            }
        }
    }
}
