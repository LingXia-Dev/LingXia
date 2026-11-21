//! Break opportunities inside addresses and URLs, which UAX #14 treats as one unbreakable word:
//! without them a long one only breaks where `overflow-wrap` runs out of room, mid-letter.

use std::sync::atomic::{AtomicUsize, Ordering};

use parley::{LineBreakContext, LineBreakOverrideFn};

/// The override for `text`, or `None` when it holds no address or URL.
pub(crate) fn overrides(text: &str) -> Option<Box<LineBreakOverrideFn>> {
    if !text.contains(['@', '/']) && !text.contains("www.") {
        return None;
    }
    let chars: Vec<char> = text.chars().collect();
    let decisions = decisions(&chars);
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

/// Per char index `i`, the decision for the pair `(i - 1, i)`: inside an address or URL, a break
/// after `@`, `-` or a run of `/`, and before `.`, unless it would leave one or two letters alone
/// at the end; every other pair inside it is closed. Text outside those tokens defers to UAX #14.
fn decisions(chars: &[char]) -> Vec<Option<bool>> {
    let mut out = vec![None; chars.len()];
    let mut start = 0;
    while start < chars.len() {
        if chars[start].is_whitespace() {
            start += 1;
            continue;
        }
        let end = (start..chars.len())
            .find(|&i| chars[i].is_whitespace())
            .unwrap_or(chars.len());
        let token = &chars[start..end];
        if is_address(token) {
            // Letters from each position to the token's end, counted from the back.
            let mut tail = vec![0usize; token.len() + 1];
            for i in (0..token.len()).rev() {
                tail[i] = tail[i + 1] + usize::from(token[i].is_alphanumeric());
            }
            let mut earlier = false;
            for i in 1..token.len() {
                let open = match (token[i - 1], token[i]) {
                    ('@', a) | ('-', a) => a.is_alphanumeric(),
                    ('/', '/') => false,
                    ('/', _) => true,
                    (b, '.') => b.is_alphanumeric(),
                    _ => false,
                };
                out[start + i] = Some(open && (!earlier || tail[i] > 2));
                earlier |= open;
            }
        }
        start = end;
    }
    out
}

/// An email address or a URL: `a@b`, `scheme://…`, `www.…`.
fn is_address(token: &[char]) -> bool {
    let at = token
        .windows(3)
        .any(|w| w[0].is_alphanumeric() && w[1] == '@' && w[2].is_alphanumeric());
    let scheme = token.windows(3).any(|w| w == [':', '/', '/']);
    let www = token.len() > 4
        && token[..4]
            .iter()
            .collect::<String>()
            .eq_ignore_ascii_case("www.");
    at || scheme || www
}

#[cfg(test)]
mod tests {
    use super::decisions;

    fn opportunities(text: &str) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        let out = decisions(&chars);
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
    fn an_address_breaks_after_the_at_and_before_each_dot() {
        assert_eq!(
            opportunities("ana@example.com"),
            ["ana@", "example", ".com"]
        );
        // A two-letter top-level domain is not left alone when an earlier break exists.
        assert_eq!(opportunities("ana@example.co"), ["ana@", "example.co"]);
        assert_eq!(opportunities("a@b.io"), ["a@", "b.io"]);
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
        assert!(decisions(&chars).iter().all(Option::is_none));
    }
}
