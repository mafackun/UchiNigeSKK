use crate::tables::ROMAJI_TO_HIRAGANA;

pub enum KanaMatch<'a> {
    Success(KanaItem<'a>),
    PrefixMatch,
    Failure,
}

pub struct KanaItem<'a> {
    pub romaji: &'a str,
    pub commit: &'a str,
    pub pushback: &'a str,
}

pub fn search_lookup_table(romaji: &str) -> KanaMatch<'static> {
    if romaji.is_empty() {
        return KanaMatch::Failure;
    }

    // partition_point: 最初に table[i].0 >= romaji となる位置
    let i = ROMAJI_TO_HIRAGANA
        .partition_point(|(k, _, _)| k < &romaji);

    // 1) 完全一致
    if let Some((k, commit, pushback)) = ROMAJI_TO_HIRAGANA.get(i) {
        if *k == romaji {
            return KanaMatch::Success(KanaItem {
                romaji: k,
                commit,
                pushback,
            });
        }

        // 2) Prefix 判定（次の要素だけ見れば十分）
        if k.starts_with(romaji) {
            return KanaMatch::PrefixMatch;
        }
    }

    KanaMatch::Failure
}

