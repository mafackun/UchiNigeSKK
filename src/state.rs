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
    Abbrev{ s: String },
}

impl KanaState {
    pub fn new_hiragana() -> Self { Self::Hiragana(false) }
    pub fn new_katakana() -> Self { Self::Katakana(false) }
}

impl InputState {
    pub fn new_latin() -> Self { Self::Latin(false) }
    pub fn new_kana() -> Self { Self::Kana { romaji: String::new(), state: KanaState::new_hiragana() } }
    pub fn new_abbrev() -> Self { Self::Abbrev{ s: String::new() } }
    pub fn new_converting(yomi: &str, jisyo: &Jisyo) -> Option<Self> {
        match jisyo.lookup(yomi) {
            Some(candidates) => Some(Self::Converting {yomi:yomi.to_string(), candidates, selected_index:0 }),
            None => None,
        }
    }
    pub fn candidate(candidates: &[String], selected_index: usize) -> (&str, Option<&str>) {
        let cand = &candidates.get(selected_index).map(|s| s.as_str()).expect("no candidates");
        let mut it = cand.splitn(2, ';');
        (it.next().unwrap(), it.next())
    }
    pub fn okuri(yomi: &str) -> Option<char> {
        if yomi.is_ascii() { return None };
        match yomi.chars().last() {
            Some(c) if c.is_ascii_lowercase() => Some(c),
            _ => None,
        }
    }
}

const LABEL_H_Z: [&str; 2] = ["半角", "全角"];

use std::fmt::{Display, Formatter, Result};
impl Display for KanaState {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            Self::Hiragana(zenkaku) => {
                let hankakuzenkaku = if *zenkaku {LABEL_H_Z[1]} else {LABEL_H_Z[0]};
                write!(f, "かな/{}記号 ", hankakuzenkaku)
            }
            Self::Katakana(hankaku) => {
                let hankakuzenkaku = if *hankaku {LABEL_H_Z[0]} else {LABEL_H_Z[1]};
                write!(f, "カナ/{} ", hankakuzenkaku)
            },
            Self::ToBeConverted(yomi) => write!(f, "かな ▽{}", yomi),
        }
    }
}

impl Display for InputState {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            Self::Abbrev{s} => write!(f, " aあ ▽{}", s),
            Self::Latin(zenkaku) => {
                let hankakuzenkaku = if *zenkaku {LABEL_H_Z[1]} else {LABEL_H_Z[0]};
                write!(f, "無変換/{}", hankakuzenkaku)
            },
            Self::Kana {romaji, state} => write!(f, "{}{}", state, romaji),
            Self::Converting {yomi, candidates, selected_index} => {
                let (cand, annotation) = InputState::candidate(&candidates, *selected_index);
                write!(f, "かな ▼{}", cand)?;
                if let Some(c) = InputState::okuri(yomi) { write!(f, "*{}", c)?; }
                write!(f, " [{}/{}]", *selected_index + 1, candidates.len())?;
                if let Some(annotation)=annotation { write!(f, "  註:{}", annotation)?; }
                Ok(())
            }
        }
    }
}
