pub struct Tokens<'a> {
    rest: &'a [u8],
}

pub fn tokens(line: &[u8]) -> Tokens<'_> {
    Tokens { rest: line }
}

pub fn trim(s: &[u8]) -> &[u8] {
    let mut start = 0usize;
    let mut end = s.len();
    while start < end && s[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && s[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &s[start..end]
}

impl<'a> Iterator for Tokens<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        let mut i = 0usize;
        while i < self.rest.len() && self.rest[i].is_ascii_whitespace() {
            i += 1;
        }
        self.rest = &self.rest[i..];
        if self.rest.is_empty() {
            return None;
        }
        let mut j = 0usize;
        while j < self.rest.len() && !self.rest[j].is_ascii_whitespace() {
            j += 1;
        }
        let (tok, rest) = self.rest.split_at(j);
        self.rest = rest;
        Some(tok)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn tokens_split_on_whitespace() {
        let got: Vec<&[u8]> = tokens(b"  cat  foo.txt ").collect();
        assert_eq!(got, vec![b"cat".as_slice(), b"foo.txt".as_slice()]);
    }

    #[test]
    fn tokens_empty() {
        assert_eq!(tokens(b"   ").next(), None);
        assert_eq!(tokens(b"").next(), None);
    }

    #[test]
    fn tokens_single() {
        let mut t = tokens(b"ls");
        assert_eq!(t.next(), Some(b"ls".as_slice()));
        assert_eq!(t.next(), None);
    }

    #[test]
    fn trim_both_ends() {
        assert_eq!(trim(b"  hello world \t"), b"hello world".as_slice());
        assert_eq!(trim(b""), b"".as_slice());
        assert_eq!(trim(b"x"), b"x".as_slice());
    }
}
