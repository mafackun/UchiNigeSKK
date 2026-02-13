use crate::jisyo::Jisyo;
use crate::util::push_itoa_usize_to_string;

const HANKAKU: &str = "半角";
const ZENKAKU: &str = "全角";

#[derive(Clone)]
pub enum KanaState {
    Hiragana(bool), // contains zenkaku flag for ascii characters
    Katakana(bool), // contains hankaku flag
    ToBeConverted(String),
}

#[derive(Clone)]
pub enum InputState {
    Latin(bool), // contains zenkaku flag
    Kana {
        romaji: String,
        state: KanaState,
    },
    Converting {
        yomi: String,
        candidates: Vec<String>,
        selected_index: usize,
    },
    Abbrev(String),
}

impl KanaState {
    pub fn new_hiragana() -> Self {
        Self::Hiragana(false)
    }
    pub fn new_katakana() -> Self {
        Self::Katakana(false)
    }
    pub fn status_as_string(&self) -> String {
        let mut out = String::new();
        match self {
            Self::Hiragana(zenkaku) => {
                out.push_str("かな/");
                out.push_str(if *zenkaku { ZENKAKU } else { HANKAKU });
                out.push_str("記号 ");
            }
            Self::Katakana(hankaku) => {
                out.push_str("カナ/");
                out.push_str(if *hankaku { HANKAKU } else { ZENKAKU });
                out.push(' ');
            }
            Self::ToBeConverted(yomi) => {
                out.push_str("かな ▽");
                out.push_str(yomi);
            }
        };
        out
    }
}

impl InputState {
    pub fn new_latin() -> Self {
        Self::Latin(false)
    }
    pub fn new_kana() -> Self {
        Self::Kana {
            romaji: String::new(),
            state: KanaState::new_hiragana(),
        }
    }
    pub fn new_abbrev() -> Self {
        Self::Abbrev(String::new())
    }
    pub fn new_converting(yomi: &str, jisyo: &Jisyo) -> Option<Self> {
        Some(Self::Converting {
            yomi: yomi.to_string(),
            candidates: jisyo.lookup(yomi)?,
            selected_index: 0,
        })
    }
    pub fn candidate(candidates: &[String], selected_index: usize) -> (&str, Option<&str>) {
        let cand = &candidates
            .get(selected_index)
            .map(|s| s.as_str())
            .expect("failed to get the candidate");
        let mut it = cand.splitn(2, ';');
        (it.next().unwrap(), it.next())
    }
    pub fn okuri(yomi: &str) -> Option<char> {
        if yomi.is_ascii() {
            return None;
        };
        match yomi.chars().last() {
            Some(c) if c.is_ascii_lowercase() => Some(c),
            _ => None,
        }
    }

    pub fn status_as_string(&self) -> String {
        let mut out = String::new();
        match self {
            Self::Abbrev(s) => {
                out.push_str(" aあ ▽");
                out.push_str(s);
            }
            Self::Latin(zenkaku) => {
                out.push_str("無変換/");
                out.push_str(if *zenkaku { ZENKAKU } else { HANKAKU });
            }
            Self::Kana { romaji, state } => {
                out.push_str(&state.status_as_string());
                out.push_str(romaji);
            }
            Self::Converting {
                yomi,
                candidates,
                selected_index,
            } => {
                let (cand, annotation) = InputState::candidate(candidates, *selected_index);
                out.push_str("かな ▼");
                out.push_str(cand);
                if let Some(c) = InputState::okuri(yomi) {
                    out.push('*');
                    out.push(c);
                }
                out.push_str(" [");
                push_itoa_usize_to_string(&mut out, *selected_index + 1, 10);
                out.push('/');
                push_itoa_usize_to_string(&mut out, candidates.len(), 10);
                out.push(']');
                if let Some(annotation) = annotation {
                    out.push_str(" 註:");
                    out.push_str(annotation);
                }
            }
        };
        out
    }
}
