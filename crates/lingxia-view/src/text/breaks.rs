//! Break opportunities inside addresses and URLs, which UAX #14 treats as one unbreakable word:
//! without them a long one only breaks where `overflow-wrap` runs out of room, mid-letter.
//!
//! An email address breaks in rungs, so its parts read as one value: first only before its `@`;
//! a paragraph whose lines still overflow is shaped again with the domain's dots open too
//! ([`Rung::Dots`]), its `@` still starting a line, and `overflow-wrap` remains the last resort.

use std::sync::atomic::{AtomicUsize, Ordering};

use parley::{LineBreakContext, LineBreakOverrideFn};

/// Which of an email address's breaks are open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Rung {
    /// Only before the `@`: the local part and the domain each stay whole.
    At,
    /// Also before each dot of the domain.
    Dots,
}

/// Each email address in `text`, as the byte offsets of its `@` and of its end: the spans
/// whose breaks come in rungs.
pub(crate) fn email_spans(text: &str) -> Vec<(usize, usize)> {
    if !text.contains('@') {
        return Vec::new();
    }
    let chars: Vec<char> = text.chars().collect();
    let bytes: Vec<usize> = text
        .char_indices()
        .map(|(at, _)| at)
        .chain([text.len()])
        .collect();
    tokens(&chars)
        .filter(|&(start, end)| is_email(&chars[start..end]))
        .filter_map(|(start, end)| {
            let at = (start..end).find(|&i| chars[i] == '@')?;
            Some((bytes[at], bytes[end]))
        })
        .collect()
}

/// The override for `text` at `rung`, or `None` when it holds no address or URL.
pub(crate) fn overrides(text: &str, rung: Rung) -> Option<Box<LineBreakOverrideFn>> {
    if !text.contains(['@', '/']) && !text.contains("www.") {
        return None;
    }
    let chars: Vec<char> = text.chars().collect();
    let decisions = decisions(&chars, rung);
    if decisions.iter().all(Option::is_none) {
        return None;
    }
    // Parley asks for every adjacent pair in order, skipping only pairs around a mandatory
    // break; those always hold a break character, so the next matching pair is the one asked.
    let cursor = AtomicUsize::new(1);
    Some(Box::new(move |cx: LineBreakContext| {
        let mut at = if cx.before_before.is_none() {
            1
        } else {
            cursor.load(Ordering::Relaxed)
        };
        while at < chars.len() {
            if chars[at - 1] == cx.before && chars[at] == cx.after {
                cursor.store(at + 1, Ordering::Relaxed);
                return decisions[at];
            }
            at += 1;
        }
        None
    }))
}

/// The whitespace-separated tokens of `chars`, as index ranges.
fn tokens(chars: &[char]) -> impl Iterator<Item = (usize, usize)> + '_ {
    let mut start = 0;
    std::iter::from_fn(move || {
        while start < chars.len() && chars[start].is_whitespace() {
            start += 1;
        }
        if start >= chars.len() {
            return None;
        }
        let end = (start..chars.len())
            .find(|&i| chars[i].is_whitespace())
            .unwrap_or(chars.len());
        let token = (start, end);
        start = end;
        Some(token)
    })
}

/// Per char index `i`, the decision for the pair `(i - 1, i)`. In a URL: a break after `@`, `-`
/// or a run of `/`, and before `.`. In an email address: before the `@`, and at [`Rung::Dots`]
/// before each dot of the domain. A break that would leave one or two letters alone at the end
/// is dropped when an earlier break exists; every other pair inside the token is closed.
/// Text outside those tokens defers to UAX #14.
fn decisions(chars: &[char], rung: Rung) -> Vec<Option<bool>> {
    let mut out = vec![None; chars.len()];
    for (start, end) in tokens(chars) {
        let token = &chars[start..end];
        let url = is_url(token);
        if !url && !is_email(token) {
            continue;
        }
        // Letters from each position to the token's end, counted from the back.
        let mut tail = vec![0usize; token.len() + 1];
        for i in (0..token.len()).rev() {
            tail[i] = tail[i + 1] + usize::from(token[i].is_alphanumeric());
        }
        let mut earlier = false;
        let mut domain = false;
        for i in 1..token.len() {
            let open = if url {
                match (token[i - 1], token[i]) {
                    ('@', a) | ('-', a) => a.is_alphanumeric(),
                    ('/', '/') => false,
                    ('/', _) => true,
                    (b, '.') => b.is_alphanumeric(),
                    _ => false,
                }
            } else {
                match (token[i - 1], token[i]) {
                    (b, '@') if !domain => {
                        domain = true;
                        b.is_alphanumeric() || matches!(b, '.' | '_' | '-' | '+')
                    }
                    (b, '.') => domain && rung == Rung::Dots && b.is_alphanumeric(),
                    _ => false,
                }
            };
            out[start + i] = Some(open && (!earlier || tail[i] > 2));
            earlier |= open;
        }
    }
    out
}

/// A URL: `scheme://…` or `www.…`.
fn is_url(token: &[char]) -> bool {
    let scheme = token.windows(3).any(|w| w == [':', '/', '/']);
    let www = token.len() > 4
        && token[..4]
            .iter()
            .collect::<String>()
            .eq_ignore_ascii_case("www.");
    scheme || www
}

/// An email address: `a@b` in a token that is no URL.
fn is_email(token: &[char]) -> bool {
    !is_url(token)
        && token
            .windows(3)
            .any(|w| w[0].is_alphanumeric() && w[1] == '@' && w[2].is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::{Rung, decisions};

    fn opportunities(text: &str) -> Vec<String> {
        rung(text, Rung::Dots)
    }

    fn rung(text: &str, rung: Rung) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        let out = decisions(&chars, rung);
        let mut pieces = Vec::new();
        let mut from = 0;
        for (i, decision) in out.iter().enumerate() {
            if *decision == Some(true) {
                pieces.push(chars[from..i].iter().collect());
                from = i;
            }
        }
        pieces.push(chars[from..].iter().collect());
        pieces
    }

    #[test]
    fn an_address_breaks_before_the_at_first_and_then_before_the_domain_dots() {
        assert_eq!(
            rung("ana.maria@example.com", Rung::At),
            ["ana.maria", "@example.com"]
        );
        assert_eq!(
            rung("ana.maria@example.com", Rung::Dots),
            ["ana.maria", "@example", ".com"]
        );
        // A two-letter top-level domain is not left alone when an earlier break exists.
        assert_eq!(opportunities("ana@example.co"), ["ana", "@example.co"]);
        assert_eq!(opportunities("a@b.io"), ["a", "@b.io"]);
    }

    #[test]
    fn a_url_breaks_after_each_slash_run_and_before_dots() {
        assert_eq!(
            opportunities("https://docs.example.com/guide/long-page"),
            [
                "https://", "docs", ".example", ".com/", "guide/", "long-", "page"
            ]
        );
        // A trailing sentence period stays on its word.
        assert_eq!(
            opportunities("See www.example.com."),
            ["See www", ".example", ".com."]
        );
    }

    #[test]
    fn plain_words_and_fractions_defer_to_unicode() {
        let chars: Vec<char> = "1/2 and/or e.g. done".chars().collect();
        assert!(decisions(&chars, Rung::Dots).iter().all(Option::is_none));
    }
}
