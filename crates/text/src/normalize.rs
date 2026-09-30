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

// Keep these lexer classes in sync with packages/markup/legacy.ts, including
// the site's historical hentaikana range and longest-token rule.
fn non_jp(ch: char) -> bool {
    matches!(ch, '.' | '/')
        || !matches!(ch, '\n' | '#' | '\u{002d}'..='\u{2010}' | '\u{2015}' | '\u{2212}'
            | '■'..='□' | '《'..='》' | '【'..='】' | '〔'..='〕'
            | '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}' | '\u{3400}'..='\u{9fea}'
            | '＃' | '％' | '（' | '）' | '／' | '＜' | '＞' | '［' | '］' | '｛'..='｝')
}
fn lexer_token(input: &[(char, Span)]) -> usize {
    let classes: [fn(char) -> bool; 5] = [
        non_jp,
        |c| matches!(c, '\u{2e80}'..='\u{2fdf}' | '\u{3003}'..='\u{3007}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}'),
        |c| matches!(c, '\u{3040}'..='\u{309f}'),
        |c| matches!(c, '\u{30a0}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}'),
        |c| matches!(c, '\u{0030}'..='\u{1b12}'),
    ];
    classes
        .into_iter()
        .map(|class| input.iter().take_while(|(c, _)| class(*c)).count())
        .max()
        .unwrap_or(0)
}
fn valid_reading(input: &[(char, Span)]) -> bool {
    !input.is_empty()
        && input
            .iter()
            .all(|item| lexer_token(std::slice::from_ref(item)) != 0)
}
fn returning(ch: char) -> bool {
    "レ一二三四五六七八九十上中下甲乙丙丁天地人".contains(ch)
}
/// Length of a return mark after `＿`: one mark, optionally followed by レ.
fn return_mark(rest: &[(char, Span)]) -> usize {
    match rest.first() {
        Some(('レ', _)) => 1,
        Some((c, _)) if returning(*c) => {
            1 + usize::from(rest.get(1).is_some_and(|(c, _)| *c == 'レ'))
        }
        _ => 0,
    }
}
fn line_break(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}
fn replace_base(base: &[(char, Span)], reading: &[(char, Span)]) -> Mapped {
    match (base.first(), base.last()) {
        (Some((_, first)), Some((_, last))) => {
            let span = Span {
                start: first.start,
                end: last.end,
            };
            reading.iter().map(|(ch, _)| (*ch, span)).collect()
        }
        _ => Vec::new(),
    }
}
// A recognized but invalid ruby consumes its entire raw token, just as parseLine does.
fn legacy_ruby(input: &[(char, Span)], side: Option<usize>) -> Option<(usize, Mapped)> {
    let prefix = usize::from(input.first()?.0 == '／');
    let rest = &input[prefix..];
    let first = rest.first()?.0;
    let base_len = if first == '【' {
        rest[1..]
            .iter()
            .position(|(c, _)| matches!(c, '【' | '】'))
            .filter(|i| *i > 0 && rest[i + 1].0 == '】')
            .map_or(0, |i| i + 2)
    } else if matches!(first, '■' | '□') {
        rest.iter().take_while(|(c, _)| *c == first).count()
    } else {
        lexer_token(rest)
    };
    if base_len == 0
        || (base_len == 1 && matches!(first, '＿' | '￣' | '｜'))
        || rest.get(base_len)?.0 != '（'
    {
        return None;
    }
    let Some(close) = rest[base_len + 1..]
        .iter()
        .position(|(c, _)| *c == '）')
        .map(|i| i + base_len + 1)
    else {
        return Some((input.len(), input.to_vec()));
    };
    let size = prefix + close + 1;
    let fields: Vec<_> = rest[base_len + 1..close]
        .split(|(c, _)| *c == '｜')
        .collect();
    let base = if first == '【' {
        &rest[1..base_len - 1]
    } else {
        &rest[..base_len]
    };
    if fields.len() > 2
        || !fields.iter().all(|field| valid_reading(field))
        || (first == '【' && !valid_reading(base))
    {
        return Some((size, input[..size].to_vec()));
    }
    Some((
        size,
        side.map_or_else(
            || base.to_vec(),
            |side| replace_base(base, fields[side.min(fields.len() - 1)]),
        ),
    ))
}

pub(crate) fn stripped(source: &str, half_breaks: bool) -> Mapped {
    strip(&source_map(source), half_breaks, None)
}
fn source_map(source: &str) -> Mapped {
    source
        .chars()
        .enumerate()
        .map(|(index, ch)| {
            (
                ch,
                Span {
                    start: index as u32,
                    end: index as u32 + 1,
                },
            )
        })
        .collect()
}
fn strip(input: &[(char, Span)], half_breaks: bool, side: Option<usize>) -> Mapped {
    let mut stack: Vec<(char, Mapped)> = Vec::new();
    let mut output = Vec::new();
    let mut index = 0;
    let mut line_end = 0;
    while index < input.len() {
        if index >= line_end {
            line_end = input[index..]
                .iter()
                .position(|(c, _)| line_break(*c))
                .map_or(input.len(), |i| index + i);
        }
        let (ch, span) = input[index];
        let rest = &input[index..line_end];
        let mut size = 1;
        let text = if ch != '※'
            && let Some((length, ruby)) = legacy_ruby(rest, side)
        {
            size = length;
            ruby
        } else if let Some(close) = match ch {
            '〔' => Some('〕'),
            '｛' => Some('｝'),
            '＜' => Some('＞'),
            _ => None,
        } && let Some(end) = rest[1..]
            .iter()
            .position(|(c, _)| *c == ch || *c == close)
            .map(|i| i + 1)
            && rest[end].0 == close
        {
            size = end + 1;
            let content = &rest[1..end];
            if ch == '｛'
                && content.first().is_some_and(|(c, _)| *c == '＿')
                && content.len() > 1
                && return_mark(&content[1..]) == content.len() - 1
            {
                Vec::new()
            } else {
                strip(content, half_breaks, side)
            }
        } else {
            match ch {
                '《' | '【' => {
                    // The command name and its colon are copied as they are, so a
                    // ruby base can never start inside the header.
                    let header = if ch == '《' {
                        rest[1..]
                            .iter()
                            .take_while(|(c, _)| !matches!(c, '《' | '》'))
                            .position(|(c, _)| *c == '：')
                            .map_or(0, |i| i + 1)
                    } else {
                        0
                    };
                    stack.push((ch, rest[1..1 + header].to_vec()));
                    index += 1 + header;
                    continue;
                }
                '》' | '】' => {
                    let opening = if ch == '》' { '《' } else { '【' };
                    if stack.last().is_some_and(|(kind, _)| *kind == opening) {
                        stack
                            .pop()
                            .map(|(kind, body)| reduce(kind, body, half_breaks, side))
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    }
                }
                '※' => {
                    size = line_end - index;
                    Vec::new()
                }
                '＃' => {
                    size += rest[1..]
                        .iter()
                        .take_while(|(c, _)| c.is_ascii_digit() || ('０'..='９').contains(c))
                        .count();
                    if size > 1 {
                        Vec::new()
                    } else {
                        vec![(ch, span)]
                    }
                }
                '＿' if return_mark(&rest[1..]) > 0 => {
                    size = 1 + return_mark(&rest[1..]);
                    Vec::new()
                }
                '￣' if rest.get(1).is_some_and(|(c, _)| ('ァ'..='ヶ').contains(c)) => {
                    size += rest[1..]
                        .iter()
                        .take_while(|(c, _)| ('ァ'..='ヶ').contains(c))
                        .count();
                    Vec::new()
                }
                '／' => Vec::new(),
                _ => {
                    let run = lexer_token(rest);
                    size = rest[..run]
                        .iter()
                        .take_while(|(c, _)| !"《》【】■□〓＃※＿￣／（）｜".contains(*c))
                        .count()
                        .max(1);
                    input[index..index + size].to_vec()
                }
            }
        };
        index += size;
        if let Some((_, body)) = stack.last_mut() {
            body.extend(text);
        } else {
            output.extend(text);
        }
    }
    while let Some((kind, body)) = stack.pop() {
        let text = reduce(kind, body, half_breaks, side);
        if let Some((_, parent)) = stack.last_mut() {
            parent.extend(text);
        } else {
            output.extend(text);
        }
    }
    output
}
fn reduce(kind: char, body: Mapped, half_breaks: bool, side: Option<usize>) -> Mapped {
    if kind == '【' {
        let name: String = body.iter().map(|(ch, _)| ch).collect();
        return if half_breaks && crate::is_section_name(&name) {
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
        "振り仮名" => {
            let base = &text[..pipe.unwrap_or(text.len())];
            if let (Some(side), Some(pipe)) = (side, pipe)
                && !text.iter().any(|(c, _)| line_break(*c))
            {
                let readings: Vec<_> = text[pipe + 1..].split(|(c, _)| *c == '｜').collect();
                replace_base(base, readings[side.min(readings.len() - 1)])
            } else {
                base.to_vec()
            }
        }
        "圏点" => text[..pipe.unwrap_or(text.len())].to_vec(),
        "返り点" | "送り仮名" => Vec::new(),
        "見せ消ち" => pipe.map(|p| text[p + 1..].to_vec()).unwrap_or_default(),
        "割書" => text.iter().copied().filter(|(ch, _)| *ch != '｜').collect(),
        _ => text.to_vec(),
    }
}
pub fn strict(source: &str) -> Normalized {
    normalize_mapped(source, stripped(source, false))
}
/// Alternate line text with ruby bases replaced by right or left readings.
/// Each reading scalar points to the entire base's original source span.
pub fn readings(source: &str) -> Vec<Normalized> {
    let input = source_map(source);
    let mut result = Vec::new();
    for side in 0..2 {
        let normalized = normalize_mapped(source, strip(&input, false, Some(side)));
        if !result.contains(&normalized) {
            result.push(normalized);
        }
    }
    result
}
fn normalize_mapped(source: &str, mapped: Mapped) -> Normalized {
    let mut visible = mapped.into_iter().peekable();
    let mut output = Vec::new();
    for (index, ch) in source.chars().enumerate() {
        while visible
            .peek()
            .is_some_and(|(_, span)| span.start as usize == index)
        {
            if let Some((ch, span)) = visible.next()
                && !ch.is_whitespace()
            {
                output.push((ch, span));
            }
        }
        if line_break(ch) {
            output.push((
                SEPARATOR,
                Span {
                    start: index as u32,
                    end: index as u32 + 1,
                },
            ));
        }
    }
    collect(output)
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
    #[test]
    fn inline_forms_follow_legacy_grammar() {
        for (source, visible, reading) in [
            ("富（ふ）士（じ）山（さん）", "富士山", "ふじさん"),
            ("／東京（とうきょう）", "東京", "とうきょう"),
            ("ひらカナ（x）", "ひらカナ", "ひらx"),
            ("説明（日本語）", "説明", "日本語"),
            ("abc（ruby）", "abc", "ruby"),
            ("かな（x）", "かな", "x"),
            ("未（a＿レ）", "未", "a＿レ"),
            ("未（a￣ヲ）", "未", "a￣ヲ"),
            ("未（a※b）", "未", "a※b"),
            ("【注】（x）", "注", "x"),
            ("■■（x）□（y）", "■■□", "xy"),
            ("之＿レ之￣ヲ之｛＿一｝", "之之之", "之之之"),
            ("〔江戸〕｛人名｝＜日時＞", "江戸人名日時", "江戸人名日時"),
            ("〔富（ふ）士（じ）〕", "富士", "ふじ"),
            ("山／川", "山川", "山川"),
            ("＿四￣ひら", "￣ひら", "￣ひら"),
            ("《振り仮名：蝦夷｜えぞ》", "蝦夷", "えぞ"),
            ("《割書：富（ふ）｜士（じ）》", "富士", "ふじ"),
            ("前《題：𠮷（よし）》後", "前𠮷後", "前よし後"),
        ] {
            assert_eq!(strict(source).text, visible, "{source}");
            assert_eq!(readings(source)[0].text, reading, "{source}");
        }
        for source in [
            "（説明）",
            "未（）",
            "未（a｜）",
            "未（a＃b）",
            "未（a《b》）",
            "未（a｜b｜c）",
            "未（a",
            "〔江戸",
            "｛人名",
            "＜日時",
        ] {
            assert_eq!(strict(source).text, source, "{source}");
            assert_eq!(readings(source)[0].text, source, "{source}");
        }
        // A base starts at the beginning of a lexer run, even when its classes overlap.
        assert_eq!(strict("㐀神（x）").text, "㐀神");
        assert_eq!(readings("㐀神（x）")[0].text, "x");
        for source in ["未（いまだ｜ズ）", "《振り仮名：未｜いまだ｜ズ》"] {
            let readings = readings(source);
            assert_eq!(
                readings.iter().map(|n| n.text.as_str()).collect::<Vec<_>>(),
                ["いまだ", "ズ"]
            );
        }
    }
    #[test]
    fn ruby_offsets_and_physical_lines() {
        let source = "𛀁富（ふ）士（じ）山（さん）";
        let visible = strict(source);
        assert_eq!(visible.text, "𛀁富士山");
        assert_eq!(
            visible.offsets,
            [
                Span { start: 0, end: 1 },
                Span { start: 1, end: 2 },
                Span { start: 5, end: 6 },
                Span { start: 9, end: 10 }
            ]
        );
        let reading = &readings(source)[0];
        assert_eq!(reading.text, "𛀁ふじさん");
        assert_eq!(
            reading.offsets,
            [
                Span { start: 0, end: 1 },
                Span { start: 1, end: 2 },
                Span { start: 5, end: 6 },
                Span { start: 9, end: 10 },
                Span { start: 9, end: 10 }
            ]
        );
        let bracket = &readings("《振り仮名：蝦夷｜えぞ》")[0];
        assert_eq!(bracket.offsets, [Span { start: 6, end: 8 }; 2]);
        for separator in ["\n", "\r\n", "\u{2028}", "\u{2029}"] {
            let source = format!("富（ふ）{separator}士（じ）");
            assert_eq!(
                strict(&source).text,
                format!("富{}士", "\0".repeat(separator.chars().count()))
            );
            assert_eq!(
                readings(&source)[0].text,
                format!("ふ{}じ", "\0".repeat(separator.chars().count()))
            );
        }
        assert_eq!(strict("富（ふ\n）士（じ）").text, "富（ふ\0）士");
        assert_eq!(readings("《振り仮名：山｜や\nま》川")[0].text, "山\0川");
    }
    proptest! {
        #[test]
        fn reading_offsets_are_monotone(source in "[富士山ふじさん《》振り仮名：（）｜〔〕｛｝＜＞＿レ￣ヲ／※＃\\n]{0,200}") {
            let length = source.chars().count() as u32;
            for normalized in readings(&source) {
                prop_assert_eq!(normalized.text.chars().count(), normalized.offsets.len());
                prop_assert!(normalized.offsets.iter().all(|s| s.start < s.end && s.end <= length));
                prop_assert!(normalized.offsets.windows(2).all(|w| w[0].start <= w[1].start));
            }
        }
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
