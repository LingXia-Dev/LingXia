//! HLS master playlists, for players that do not list their renditions or pin them themselves.

/// One video height of a master playlist and the highest bandwidth any variant gives it.
#[derive(Clone, Debug, PartialEq)]
pub struct Rendition {
    /// The height as a quality label, such as `720p`.
    pub label: String,
    pub bandwidth: u32,
    /// The variant's media playlist, as the master names it (possibly relative).
    pub uri: String,
}

/// Whether `url`'s path names a playlist, so its renditions are worth reading.
pub fn is_playlist(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    (url.starts_with("https://") || url.starts_with("http://"))
        && path.to_ascii_lowercase().ends_with(".m3u8")
}

/// The video renditions of a master playlist, highest first, one per height; empty for a
/// media playlist or a master whose variants give no resolution.
pub fn renditions(playlist: &str) -> Vec<Rendition> {
    let mut out: Vec<(u32, Rendition)> = Vec::new();
    let mut lines = playlist.lines().map(str::trim);
    while let Some(line) = lines.next() {
        let Some(attributes) = line.strip_prefix("#EXT-X-STREAM-INF:") else {
            continue;
        };
        // The URI line follows its tag, after any blank or comment lines.
        let Some(uri) = lines
            .by_ref()
            .find(|l| !l.is_empty() && !l.starts_with('#'))
        else {
            continue;
        };
        let (mut height, mut bandwidth) = (None, None);
        for (name, value) in attribute_list(attributes) {
            match name {
                "RESOLUTION" => {
                    height = value
                        .split_once(['x', 'X'])
                        .and_then(|(_, h)| h.parse::<u32>().ok())
                }
                "BANDWIDTH" => bandwidth = value.parse::<u32>().ok(),
                _ => {}
            }
        }
        let (Some(height), Some(bandwidth)) = (height.filter(|h| *h > 0), bandwidth) else {
            continue;
        };
        match out.iter_mut().find(|(h, _)| *h == height) {
            Some((_, existing)) if existing.bandwidth >= bandwidth => {}
            Some((_, existing)) => {
                existing.bandwidth = bandwidth;
                existing.uri = uri.to_owned();
            }
            None => out.push((
                height,
                Rendition {
                    label: format!("{height}p"),
                    bandwidth,
                    uri: uri.to_owned(),
                },
            )),
        }
    }
    out.sort_by_key(|r| std::cmp::Reverse(r.0));
    out.into_iter().map(|(_, rendition)| rendition).collect()
}

/// `uri` of a playlist at `base`: absolute as given, else against the base's path or host.
pub fn resolve(base: &str, uri: &str) -> String {
    if uri.starts_with("https://") || uri.starts_with("http://") {
        return uri.to_owned();
    }
    let base = base.split(['?', '#']).next().unwrap_or(base);
    if let Some(path) = uri.strip_prefix('/') {
        let origin_end = base
            .find("://")
            .and_then(|s| base[s + 3..].find('/').map(|p| s + 3 + p))
            .unwrap_or(base.len());
        return format!("{}/{path}", &base[..origin_end]);
    }
    match base.rfind('/') {
        Some(slash) => format!("{}/{uri}", &base[..slash]),
        None => uri.to_owned(),
    }
}

/// `NAME=value` pairs; a quoted value may hold commas.
fn attribute_list(text: &str) -> Vec<(&str, &str)> {
    let mut pairs = Vec::new();
    let mut rest = text;
    while let Some((name, after)) = rest.split_once('=') {
        let (value, next) = match after.strip_prefix('"') {
            Some(quoted) => {
                let end = quoted.find('"').unwrap_or(quoted.len());
                let tail = &quoted[(end + 1).min(quoted.len())..];
                (&quoted[..end], tail.split_once(',').map_or("", |(_, t)| t))
            }
            None => after.split_once(',').unwrap_or((after, "")),
        };
        pairs.push((name.trim(), value));
        rest = next;
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_master_playlist_gives_one_rendition_per_height_at_its_highest_bandwidth() {
        let master = "#EXTM3U\n\
            #EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS=\"avc1.4d401f,mp4a.40.2\",RESOLUTION=640x360\n360.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=6000000,RESOLUTION=1920x1080,FRAME-RATE=30\n1080.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=1200000,RESOLUTION=640x360,CODECS=\"hvc1.1.6.L93.90\"\n360h.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=64000,CODECS=\"mp4a.40.2\"\naudio.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=3000000,RESOLUTION=1280x720\n720.m3u8\n";
        let got: Vec<_> = renditions(master)
            .into_iter()
            .map(|r| (r.label, r.bandwidth, r.uri))
            .collect();
        assert_eq!(
            got,
            [
                ("1080p".to_owned(), 6_000_000, "1080.m3u8".to_owned()),
                ("720p".to_owned(), 3_000_000, "720.m3u8".to_owned()),
                ("360p".to_owned(), 1_200_000, "360h.m3u8".to_owned()),
            ]
        );
    }

    #[test]
    fn a_variant_uri_resolves_against_its_master() {
        let master = "https://cdn.example/a/b/master.m3u8?token=1";
        assert_eq!(
            resolve(master, "gear3/prog_index.m3u8"),
            "https://cdn.example/a/b/gear3/prog_index.m3u8"
        );
        assert_eq!(
            resolve(master, "/v/540.m3u8"),
            "https://cdn.example/v/540.m3u8"
        );
        assert_eq!(
            resolve(master, "https://other.example/x.m3u8"),
            "https://other.example/x.m3u8"
        );
    }

    #[test]
    fn a_media_playlist_has_no_renditions_and_only_m3u8_urls_are_read() {
        assert!(renditions("#EXTM3U\n#EXTINF:6.0,\nsegment0.ts\n").is_empty());
        assert!(is_playlist("https://cdn.example/a/master.M3U8?token=1"));
        assert!(!is_playlist("https://cdn.example/a/clip.mp4"));
        assert!(!is_playlist("file:///data/master.m3u8"));
    }
}
