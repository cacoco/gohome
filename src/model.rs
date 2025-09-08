use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Link is the structure stored for each go short link.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Link {
    pub id: String,    // normalized short key Id
    pub short: String, // the user-provided "foo" part of "http://go/foo"
    pub long: String,  // the target URL or text/template pattern to run
    pub created: chrono::DateTime<Utc>,
    pub updated: chrono::DateTime<Utc>,
    pub owner: Option<String>,
}

impl std::fmt::Display for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: ", self.id)?;
        write!(f, "go/{} -> {}", self.short, self.long)?;
        // If an owner exists, append it to the string.
        if let Some(owner) = &self.owner {
            write!(f, " (owner: {})", owner)?;
        }
        write!(f, " [created: {}, updated: {}]", self.created, self.updated)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClickStats {
    pub id: String, // normalized short key Id
    pub created: chrono::DateTime<Utc>,
    pub clicks: Option<i32>, // number of times link has been clicked
}

/// returns the normalized Id for a link short name.
pub fn normalized_id(short: &str) -> String {
    urlencoding::encode(short).replace('-', "to")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test case 1: A simple string with no special characters or hyphens.
    #[test]
    fn test_simple_string() {
        let input = "hello";
        let expected = "hello".to_string();
        assert_eq!(normalized_id(input), expected);
    }

    // Test case 2: A string containing a hyphen, which should be replaced.
    #[test]
    fn test_with_hyphen() {
        let input = "hello-world";
        // The hyphen should be replaced with "to".
        let expected = "hellotoworld".to_string();
        assert_eq!(normalized_id(input), expected);
    }

    // Test case 3: A string with a space, which requires URL encoding.
    #[test]
    fn test_with_space_encoding() {
        let input = "hello world";
        // The space should be encoded to "%20".
        let expected = "hello%20world".to_string();
        assert_eq!(normalized_id(input), expected);
    }

    // Test case 4: A string with multiple special characters that need encoding.
    #[test]
    fn test_with_special_characters() {
        let input = "a/b?c=d&e";
        // Characters like '/', '?', '=', '&' should be percent-encoded.
        let expected = "a%2Fb%3Fc%3Dd%26e".to_string();
        assert_eq!(normalized_id(input), expected);
    }

    // Test case 5: A string with both a hyphen and special characters.
    #[test]
    fn test_with_hyphen_and_special_chars() {
        let input = "rust-lang/book";
        // The hyphen is replaced first, then the string is encoded.
        // Oh, wait, the function encodes *first*, then replaces. Let's trace:
        // 1. urlencoding::encode("rust-lang/book") -> "rust-lang%2Fbook"
        // 2. "rust-lang%2Fbook".replace('-', "to") -> "rusttolang%2Fbook"
        let expected = "rusttolang%2Fbook".to_string();
        assert_eq!(normalized_id(input), expected);
    }

    // Test case 6: An empty string.
    #[test]
    fn test_empty_string() {
        let input = "";
        let expected = "".to_string();
        assert_eq!(normalized_id(input), expected);
    }

    // Test case 7: A string with multiple hyphens.
    #[test]
    fn test_multiple_hyphens() {
        let input = "a-b-c";
        let expected = "atobtoc".to_string();
        assert_eq!(normalized_id(input), expected);
    }
}
