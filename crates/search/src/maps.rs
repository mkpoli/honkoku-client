use crate::{Error, Result};
use honkoku_text::normalize::Span;

/// Delta starts and span lengths, encoded as unsigned LEB128.
pub fn encode(spans: &[Span]) -> Vec<u8> {
    fn put(mut n: u32, out: &mut Vec<u8>) {
        while n >= 128 {
            out.push((n as u8 & 127) | 128);
            n >>= 7;
        }
        out.push(n as u8);
    }
    let mut out = Vec::with_capacity(spans.len() * 2);
    let mut previous = 0;
    for span in spans {
        put(span.start - previous, &mut out);
        put(span.end - span.start, &mut out);
        previous = span.start;
    }
    out
}
pub fn decode(bytes: &[u8]) -> Result<Vec<Span>> {
    fn get(bytes: &mut std::slice::Iter<'_, u8>) -> Result<u32> {
        let mut n = 0;
        for shift in (0..35).step_by(7) {
            let b = *bytes
                .next()
                .ok_or_else(|| Error::Invalid("truncated offset map".into()))?;
            if shift == 28 && b > 15 {
                return Err(Error::Invalid("offset overflow".into()));
            }
            n |= u32::from(b & 127) << shift;
            if b < 128 {
                return Ok(n);
            }
        }
        Err(Error::Invalid("invalid offset map".into()))
    }
    let mut bytes = bytes.iter();
    let mut out = Vec::new();
    let mut previous = 0u32;
    while !bytes.as_slice().is_empty() {
        let start = previous
            .checked_add(get(&mut bytes)?)
            .ok_or_else(|| Error::Invalid("offset overflow".into()))?;
        let end = start
            .checked_add(get(&mut bytes)?)
            .ok_or_else(|| Error::Invalid("offset overflow".into()))?;
        out.push(Span { start, end });
        previous = start;
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_round_trip() -> Result<()> {
        let spans = vec![
            Span { start: 0, end: 1 },
            Span {
                start: 300,
                end: 304,
            },
            Span {
                start: 300,
                end: 304,
            },
            Span {
                start: 50000,
                end: 50001,
            },
        ];
        assert_eq!(decode(&encode(&spans))?, spans);
        assert!(decode(&[128]).is_err());
        assert!(decode(&[255; 10]).is_err());
        Ok(())
    }
}
