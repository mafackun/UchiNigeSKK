use crate::jisyo::Jisyo;

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
    Abbrev {
        s: String,
    },
}

impl KanaState {
    pub fn new_hiragana() -> Self {
        Self::Hiragana(false)
    }
    pub fn new_katakana() -> Self {
        Self::Katakana(false)
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
        Self::Abbrev { s: String::new() }
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
}

const HANKAKU: &str = "半角";
const ZENKAKU: &str = "全角";

use std::fmt::{Display, Formatter, Result};
impl Display for KanaState {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            Self::Hiragana(zenkaku) => {
                write!(f, "かな/{}記号 ", if *zenkaku { ZENKAKU } else { HANKAKU })
            }
            Self::Katakana(hankaku) => {
                write!(f, "カナ/{} ", if *hankaku { HANKAKU } else { ZENKAKU })
            }
            Self::ToBeConverted(yomi) => write!(f, "かな ▽{}", yomi),
        }
    }
}

impl Display for InputState {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            Self::Abbrev { s } => write!(f, " aあ ▽{}", s),
            Self::Latin(zenkaku) => {
                write!(f, "無変換/{}", if *zenkaku { ZENKAKU } else { HANKAKU })
            }
            Self::Kana { romaji, state } => write!(f, "{}{}", state, romaji),
            Self::Converting {
                yomi,
                candidates,
                selected_index,
            } => {
                let (cand, annotation) = InputState::candidate(candidates, *selected_index);
                write!(f, "かな ▼{}", cand)?;
                if let Some(c) = InputState::okuri(yomi) {
                    write!(f, "*{}", c)?;
                }
                write!(f, " [{}/{}]", *selected_index + 1, candidates.len())?;
                if let Some(annotation) = annotation {
                    write!(f, "  註:{}", annotation)?;
                }
                Ok(())
            }
        }
    }
}
