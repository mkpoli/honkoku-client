use honkoku_text::normalize::SEPARATOR;
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

#[derive(Clone)]
pub struct Characters(pub usize);
pub struct CharacterStream<'a> {
    text: &'a str,
    chars: Vec<(usize, char)>,
    next: usize,
    width: usize,
    token: Token,
}
impl Tokenizer for Characters {
    type TokenStream<'a> = CharacterStream<'a>;
    fn token_stream<'a>(&'a mut self, text: &'a str) -> CharacterStream<'a> {
        CharacterStream {
            text,
            chars: text.char_indices().collect(),
            next: 0,
            width: self.0,
            token: Token::default(),
        }
    }
}
impl TokenStream for CharacterStream<'_> {
    fn advance(&mut self) -> bool {
        while self.next + self.width <= self.chars.len() {
            let position = self.next;
            self.next += 1;
            if self.chars[position..position + self.width]
                .iter()
                .any(|(_, c)| *c == SEPARATOR)
            {
                continue;
            }
            let start = self.chars[position].0;
            let end = self
                .chars
                .get(position + self.width)
                .map_or(self.text.len(), |(i, _)| *i);
            self.token.offset_from = start;
            self.token.offset_to = end;
            self.token.position = position;
            self.token.text.clear();
            self.token.text.push_str(&self.text[start..end]);
            return true;
        }
        false
    }
    fn token(&self) -> &Token {
        &self.token
    }
    fn token_mut(&mut self) -> &mut Token {
        &mut self.token
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn positions_and_boundaries() {
        let mut tokenizer = Characters(2);
        let mut stream = tokenizer.token_stream("𛀁日本\0日本");
        let mut tokens = Vec::new();
        while stream.advance() {
            let t = stream.token();
            tokens.push((t.text.clone(), t.position, t.offset_from, t.offset_to));
        }
        assert_eq!(
            tokens,
            vec![
                ("𛀁日".into(), 0, 0, 7),
                ("日本".into(), 1, 4, 10),
                ("日本".into(), 4, 11, 17)
            ]
        );
    }
}
