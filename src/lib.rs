pub fn ends_with_newline(bytes: &[u8]) -> bool {
    bytes.ends_with(b"\n")
}

pub fn strip_one_trailing_newline(bytes: &mut Vec<u8>) -> bool {
    if bytes.ends_with(b"\r\n") {
        let new_len = bytes.len() - 2;
        bytes.truncate(new_len);
        return true;
    }
    if bytes.ends_with(b"\n") {
        let new_len = bytes.len() - 1;
        bytes.truncate(new_len);
        return true;
    }
    false
}

pub fn added_eof_newline(old_bytes: &[u8], new_bytes: &[u8]) -> bool {
    !ends_with_newline(old_bytes) && ends_with_newline(new_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ends_with_newline_cases() {
        assert!(!ends_with_newline(b""));
        assert!(!ends_with_newline(b"a"));
        assert!(ends_with_newline(b"\n"));
        assert!(ends_with_newline(b"\r\n"));
        assert!(ends_with_newline(b"a\n"));
        assert!(ends_with_newline(b"a\r\n"));
    }

    #[test]
    fn strip_one_trailing_newline_cases() {
        let mut v = b"".to_vec();
        assert!(!strip_one_trailing_newline(&mut v));
        assert_eq!(v, b"");

        let mut v = b"a".to_vec();
        assert!(!strip_one_trailing_newline(&mut v));
        assert_eq!(v, b"a");

        let mut v = b"a\n".to_vec();
        assert!(strip_one_trailing_newline(&mut v));
        assert_eq!(v, b"a");

        let mut v = b"a\r\n".to_vec();
        assert!(strip_one_trailing_newline(&mut v));
        assert_eq!(v, b"a");
    }

    #[test]
    fn added_eof_newline_cases() {
        assert!(added_eof_newline(b"a", b"a\n"));
        assert!(added_eof_newline(b"a", b"a\r\n"));
        assert!(!added_eof_newline(b"a\n", b"a\n"));
        assert!(!added_eof_newline(b"a\n", b"a"));
        assert!(added_eof_newline(b"", b"\n"));
    }
}
