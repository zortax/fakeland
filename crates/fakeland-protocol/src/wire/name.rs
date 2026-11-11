//! Mark and span names. A name is a non-empty UTF-8 string of at most `MAX_NAME_LEN` bytes.

/// The maximum length of a name in bytes.
pub const MAX_NAME_LEN: usize = 256;

/// A name that the protocol rejects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    /// The name is empty.
    #[error("the name is empty")]
    Empty,
    /// The name is longer than `MAX_NAME_LEN` bytes.
    #[error("the name is {0} bytes long, the limit is {MAX_NAME_LEN} bytes")]
    TooLong(usize),
}

/// Checks a mark or span name.
pub fn check_name(name: &str) -> Result<(), NameError> {
    match name.len() {
        0 => Err(NameError::Empty),
        len if len > MAX_NAME_LEN => Err(NameError::TooLong(len)),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_names_up_to_the_limit() {
        assert_eq!(check_name("paint"), Ok(()));
        assert_eq!(check_name(&"x".repeat(MAX_NAME_LEN)), Ok(()));
    }

    #[test]
    fn rejects_empty_and_long_names() {
        assert_eq!(check_name(""), Err(NameError::Empty));
        assert_eq!(
            check_name(&"x".repeat(MAX_NAME_LEN + 1)),
            Err(NameError::TooLong(MAX_NAME_LEN + 1))
        );
    }

    #[test]
    fn counts_bytes_of_multibyte_text() {
        let name = "\u{00e9}".repeat(MAX_NAME_LEN / 2 + 1);
        assert_eq!(check_name(&name), Err(NameError::TooLong(MAX_NAME_LEN + 2)));
    }
}
