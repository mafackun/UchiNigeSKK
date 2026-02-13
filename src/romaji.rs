use crate::tables::ROMAJI_TO_HIRAGANA;

pub enum KanaMatch<'a> {
    Success(KanaConverted<'a>),
    PrefixMatch,
    Failure,
}

pub struct KanaConverted<'a> {
    pub commit: &'a str,
    pub pushback: &'a str,
}

pub fn search_lookup_table(romaji: &str) -> KanaMatch<'static> {
    if romaji.is_empty() {
        return KanaMatch::Failure;
    }

    let i = ROMAJI_TO_HIRAGANA.partition_point(|(k, _)| k < &romaji);

    if let Some((k, conv)) = ROMAJI_TO_HIRAGANA.get(i) {
        if *k == romaji {
            let last = conv.len() - 1;
            let (commit, pushback) = if conv.as_bytes()[last].is_ascii_lowercase() {
                (&conv[0..last], &conv[last..])
            } else {
                (*conv, "")
            };
            return KanaMatch::Success(KanaConverted { commit, pushback });
        }
        if k.starts_with(romaji) {
            return KanaMatch::PrefixMatch;
        }
    }
    KanaMatch::Failure
}
