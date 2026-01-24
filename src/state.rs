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
}

impl KanaState {
    pub fn new_hiragana() -> Self { Self::Hiragana(false) }
    pub fn new_katakana() -> Self { Self::Katakana(false) }
}

impl InputState {
    pub fn new_latin() -> Self { Self::Latin(false) }
    pub fn new_kana() -> Self { Self::Kana { romaji: String::new(), state: KanaState::new_hiragana() } }
    pub fn new_converting(yomi: &str, jisyo: &Jisyo) -> Option<Self> {
        match jisyo.lookup(yomi) {
            Some(candidates) => Some(Self::Converting {yomi:yomi.to_string(), candidates, selected_index:0 }),
            None => None,
        }
    }
}

pub fn split_candidate(cand: &str) -> (&str, Option<&str>) {
    let mut it = cand.splitn(2, ';');
    (it.next().unwrap(), it.next())
}

const LABEL_ZENKAKU_HANKAKU: [&str; 2] = ["半角", "全角"];
use std::fmt::{Display, Formatter, Result};
impl Display for KanaState {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            Self::Hiragana(zenkaku) => {
                let hankakuzenkaku = if *zenkaku {LABEL_ZENKAKU_HANKAKU[1]} else {LABEL_ZENKAKU_HANKAKU[0]};
                write!(f, "かな/{}記号 ", hankakuzenkaku)
            }
            Self::Katakana(hankaku) => {
                let hankakuzenkaku = if *hankaku {LABEL_ZENKAKU_HANKAKU[0]} else {LABEL_ZENKAKU_HANKAKU[1]};
                write!(f, "カナ/{} ", hankakuzenkaku)
            },
            Self::ToBeConverted(yomi) => write!(f, "▽{}", yomi),
        }
    }
}

impl Display for InputState {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "入力方式: ")?;
        match self {
            Self::Latin(zenkaku) => {
                let hankakuzenkaku = if *zenkaku {LABEL_ZENKAKU_HANKAKU[1]} else {LABEL_ZENKAKU_HANKAKU[0]};
                write!(f, "無変換/{}", hankakuzenkaku)
            },
            Self::Kana {romaji, state} => write!(f, "{}{}", state, romaji),
            Self::Converting {yomi, candidates, selected_index} => {
                let total = candidates.len();
                if total < 1 { panic!("no candidates"); }

                let idx = (*selected_index + 1).min(total);
                let sel = candidates.get(*selected_index).map(|s| s.as_str()).unwrap();
                let (commit, annotation) = split_candidate(&sel);
                write!(f, "▼{}", commit)?;
                if let Some(okurigana)=yomi.chars().last() && okurigana.is_ascii_lowercase() {
                    write!(f, "*{}", okurigana)?;
                }
                write!(f, " [{}/{}]", idx, total)?;
                if let Some(annotation)=annotation {
                    write!(f, "  註:{}", annotation)?;
                }
                Ok(())
            }
        }
    }
}
