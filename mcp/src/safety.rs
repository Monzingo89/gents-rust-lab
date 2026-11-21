use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DataAssessment {
    pub level: String,
    pub indicators: Vec<String>,
    pub handling: String,
    pub persisted: bool,
}

impl Default for DataAssessment {
    fn default() -> Self {
        Self {
            level: "legacy_unclassified".to_owned(),
            indicators: Vec::new(),
            handling: "retained legacy run; classify before reuse".to_owned(),
            persisted: true,
        }
    }
}

fn luhn(value: &str) -> bool {
    let digits = value
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|value| (value - b'0') as u32)
        .collect::<Vec<_>>();
    if !(13..=19).contains(&digits.len()) {
        return false;
    }
    let parity = digits.len() % 2;
    digits
        .iter()
        .enumerate()
        .map(|(index, digit)| {
            if index % 2 == parity {
                let doubled = digit * 2;
                if doubled > 9 {
                    doubled - 9
                } else {
                    doubled
                }
            } else {
                *digit
            }
        })
        .sum::<u32>()
        % 10
        == 0
}

fn looks_like_email(token: &str) -> bool {
    // Inspect address-shaped spans around every @. Treating an entire URL as
    // an address falsely classified public npm /@scope/package-1.2.3.tgz
    // references; checking only the first @ also missed later real addresses.
    token.match_indices('@').any(|(index, _)| {
        let local = token[..index]
            .chars()
            .rev()
            .take_while(|c| {
                c.is_alphanumeric()
                    || matches!(
                        c,
                        '.' | '_'
                            | '-'
                            | '+'
                            | '%'
                            | '!'
                            | '#'
                            | '$'
                            | '&'
                            | '\''
                            | '*'
                            | '='
                            | '?'
                            | '^'
                            | '`'
                            | '{'
                            | '|'
                            | '}'
                            | '~'
                    )
            })
            .collect::<String>();
        let domain = token[index + 1..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || matches!(c, '.' | '-'))
            .collect::<String>();
        !local.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.trim_end_matches('.').is_empty()
    })
}

fn looks_like_ssn(token: &str) -> bool {
    let parts = token
        .trim_matches(|character: char| !character.is_ascii_digit() && character != '-')
        .split('-')
        .collect::<Vec<_>>();
    parts.len() == 3
        && [3, 2, 4]
            .iter()
            .zip(parts)
            .all(|(length, part)| part.len() == *length && part.bytes().all(|v| v.is_ascii_digit()))
}

fn has_pan(text: &str) -> bool {
    // Match complete numeric tokens, not digit substrings inside file IDs or
    // hashes. Keep spaces/hyphens within a card number, but trim separators
    // before checking its surrounding token boundaries.
    let candidate = |start: usize, end: usize| {
        let raw = &text[start..end];
        let number = raw.trim_matches(' ');
        if number.is_empty() {
            return false;
        }
        let begin = start + raw.len() - raw.trim_start_matches(' ').len();
        let finish = begin + number.len();
        let identifier = |c: char| c.is_alphanumeric() || c == '_';
        !text[..begin].chars().next_back().is_some_and(identifier)
            && !text[finish..].chars().next().is_some_and(identifier)
            && luhn(number)
    };
    let mut start = None;
    for (index, character) in text.char_indices() {
        if character.is_ascii_digit() || matches!(character, '-' | ' ') {
            start.get_or_insert(index);
        } else if let Some(begin) = start.take() {
            if candidate(begin, index) {
                return true;
            }
        }
    }
    start.is_some_and(|begin| candidate(begin, text.len()))
}

pub fn assess(text: &str) -> DataAssessment {
    let lower = text.to_ascii_lowercase();
    let mut indicators = Vec::new();
    let has_email = text.split_whitespace().any(looks_like_email);
    let has_ssn = text.split_whitespace().any(looks_like_ssn);
    let has_pan = has_pan(text);
    if has_email {
        indicators.push("email_address".to_owned());
    }
    if has_ssn {
        indicators.push("us_ssn".to_owned());
    }
    if has_pan {
        indicators.push("payment_card_pan".to_owned());
    }
    let health = [
        "diagnosis",
        "patient",
        "medical record",
        "prescription",
        "treatment",
        "health insurance",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let level = if has_pan {
        "payment_card"
    } else if health && (has_email || has_ssn) {
        "phi"
    } else if has_email || has_ssn {
        "pii"
    } else {
        "standard"
    };
    let restricted = level != "standard";
    DataAssessment {
        level: level.to_owned(),
        indicators,
        handling: if restricted {
            "blocked_before_persistence; replace direct identifiers with approved tokens or de-identified data"
                .to_owned()
        } else {
            "local_private_default".to_owned()
        },
        persisted: !restricted,
    }
}

pub fn require_persistable(text: &str) -> anyhow::Result<DataAssessment> {
    let assessment = assess(text);
    if !assessment.persisted {
        anyhow::bail!(
            "sensitive data gate: {} detected ({}) before persistence; use tokenized or de-identified input. This harness does not claim HIPAA or PCI DSS certification",
            assessment.level,
            assessment.indicators.join(", ")
        );
    }
    Ok(assessment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pan_detection_distinguishes_standalone_values_from_identifier_substrings() {
        for value in [
            "4111111111111111",
            "card: 4111 1111 1111 1111 expires soon",
            r#"{"number":"4111-1111-1111-1111"}"#,
            "files/4111111111111111.pdf",
        ] {
            assert_eq!(assess(value).level, "payment_card", "{value}");
        }
        for value in [
            "files/4111111111111111x.pdf",
            "files/4111111111111111-reference.pdf",
            "hash_a4111111111111111b",
            "size_id4111111111111111suffix",
        ] {
            assert_eq!(assess(value).level, "standard", "{value}");
        }
    }
    #[test]
    fn classifies_without_treating_policy_words_as_live_data() {
        assert_eq!(
            assess("Review our PII, PHI, and PCI policy").level,
            "standard"
        );
        assert_eq!(assess("Contact jane@example.com").level, "pii");
        assert_eq!(
            assess("Patient diagnosis for jane@example.com").level,
            "phi"
        );
        assert_eq!(assess("4111 1111 1111 1111").level, "payment_card");
        assert!(require_persistable("4111 1111 1111 1111").is_err());
    }
    #[test]
    fn public_scoped_package_urls_are_not_addresses_but_embedded_addresses_still_are() {
        assert!(require_persistable(
            r#"{"resolved":"https://registry.npmjs.org/@angular/core/-/core-21.2.26.tgz"}"#
        )
        .is_ok());
        for address in [
            r#"{"contact":"jane@example.com"}"#,
            "@scope/package;jane@example.com",
            "https://jane@example.com/path",
            "first+last@example.com",
            "Patient diagnosis: jane@example.com.",
        ] {
            assert!(require_persistable(address).is_err(), "{address}");
        }
    }
}
