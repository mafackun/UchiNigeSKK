use crate::{
    buffer::Buffer,
    key::KeyEvent,
    romaji::{KanaMatch, search_lookup_table},
    state::{InputState, KanaState},
    jisyo::Jisyo,
    tables::HIRAGANA_TO_HALFWIDTH_KATAKANA,
};

pub fn handle_key(
    state: InputState,
    buffer: &mut Buffer,
    jisyo: &Jisyo,
    key: KeyEvent,
) -> InputState {
    match key {
        KeyEvent::MoveLeft => { _ = buffer.move_left();  return state; }
        KeyEvent::MoveRight => { _ = buffer.move_right(); return state; }
        KeyEvent::MoveUp => { _ = buffer.move_up();    return state; }
        KeyEvent::MoveDown => { _ = buffer.move_down();  return state; }
        KeyEvent::ToLineHead => { buffer.move_line_head(); return state; }
        KeyEvent::ToLineTail => { buffer.move_line_tail(); return state; }
        KeyEvent::Delete => { buffer.delete(); return state; }
        _ => {}
    }

    match state {
        InputState::Kana {romaji, state} => handle_kana(romaji, state, buffer, jisyo, key),
        InputState::Converting {yomi:y, candidates:c, selected_index:i} => handle_converting(y, c, i, buffer, jisyo, key),
        InputState::Latin(zenkaku) => handle_latin(zenkaku, buffer, key),
    }
}

// -------------------- Latin --------------------

fn handle_latin(mut is_zenkaku: bool, buffer: &mut Buffer, key: KeyEvent) -> InputState {
    use KeyEvent::{ToggleLatin, ToggleHankakuZenkaku, Char, Backspace, ToggleHKL};
    match key {
        Char(c) => buffer.insert_char(if is_zenkaku { convert_to_zenkaku_ascii(c) } else { c }),
        ToggleHankakuZenkaku => is_zenkaku = !is_zenkaku,
        Backspace => buffer.backspace(),
        ToggleLatin | ToggleHKL => return InputState::new_kana(),
        _ => (),
    }
    InputState::Latin(is_zenkaku)
}

// -------------------- Kana --------------------

#[allow(unused_parens)]
fn handle_kana(
    mut romaji: String,
    mut state: KanaState,
    buffer: &mut Buffer,
    jisyo: &Jisyo,
    key: KeyEvent,
) -> InputState {
    use KanaState::{*};
    use KeyEvent::{
        ToggleLatin, ToggleHankakuZenkaku, ToggleKatakana, ToggleHKL, StartConversion, Char, Backspace, 
        CommitUnconvertedYomi, SetsubijiSettouji, StartYomi, Okurigana
    };

    match key {
        ToggleLatin => return InputState::new_latin(),
        ToggleHankakuZenkaku => state = (match state {
            Katakana(hankaku) => Katakana(!hankaku),
            Hiragana(zenkaku) => Hiragana(!zenkaku),
            other => other,
        }),
        ToggleKatakana => state = (match state { 
            Hiragana(_) => KanaState::new_katakana(), 
            Katakana(_) => KanaState::new_hiragana(), 
            _ => KanaState::new_katakana(), 
        }),
        ToggleHKL => state = (match state {
            Hiragana(_) => KanaState::new_katakana(), 
            Katakana(_) => return InputState::new_latin(), 
            other => other,
        }),
        StartConversion => if let ToBeConverted(ref y)=state { 
            if let Some(c)=InputState::new_converting(y, jisyo) { return c; }
        },
        Backspace => {
            if !romaji.is_empty() { romaji.pop(); } 
            else if let ToBeConverted(yomi) = &mut state {
                if !yomi.is_empty() { yomi.pop(); } 
                else { state = KanaState::new_hiragana(); } } 
            else { buffer.backspace(); }
        }
        CommitUnconvertedYomi => 
            if let ToBeConverted(ref mut y)=state { buffer.insert_str(y); return InputState::new_kana(); },
        SetsubijiSettouji if romaji.is_empty() => {
            if let ToBeConverted(ref mut y) = state && !y.is_empty() {
                y.push('>');
                if let Some(c)=InputState::new_converting(y, jisyo) { return c; }
            } else { state = ToBeConverted(String::from(">")) }
        }
        StartYomi(c) if romaji.is_empty() => return handle_key( 
            InputState::Kana {romaji: String::new(), state: ToBeConverted(String::new())}, buffer, jisyo, Char(c) 
        ),
        Okurigana(c) if romaji.is_empty() => if let ToBeConverted(ref mut y)=state {
            if !y.is_empty() { 
                y.push(c); 
                if let Some(c)=InputState::new_converting(y, jisyo) { return c; } else { y.pop(); }
            } else { return handle_key(InputState::new_kana(), buffer, jisyo, StartYomi(c)); } 
        },
        Char(c) => 'char: {
            romaji.push(c);
            match search_lookup_table(&romaji) {
                KanaMatch::Success(kana) => {
                    commit_kana(buffer, &mut state, &kana.commit);
                    romaji.clear();
                    romaji.push_str(kana.pushback);
                },
                KanaMatch::Failure => {
                    romaji.pop();
                    if let ToBeConverted(_)=state { break 'char; }
                    if (c.is_ascii_punctuation()||c.is_ascii_digit()) && romaji.is_empty() { 
                        buffer.insert_char(if let Hiragana(true)=state { convert_to_zenkaku_ascii(c) } else { c }) 
                    };
                },
                KanaMatch::PrefixMatch => (), 
            }
        }
        _ => (),
    }

    InputState::Kana { romaji, state }
}

// -------------------- Converting --------------------

fn handle_converting(
    mut yomi: String,
    candidates: Vec<String>,
    mut selected_index: usize,
    buffer: &mut Buffer,
    jisyo: &Jisyo,
    key: KeyEvent,
) -> InputState {
    use KeyEvent::{
        CommitCandidate, CancelConversion, NextCandidate, PrevCandidate, Backspace, Char, StartYomi, 
        SetsubijiSettouji, CommitCandidateWithChar, CommitCandidateWithStartYomi, CommitCandidateWithSetsubiji,
        ToggleKatakana
    };
    match key {
        CancelConversion => return InputState::new_kana(),
        NextCandidate => if selected_index + 1 < candidates.len() { selected_index += 1; },
        PrevCandidate => if selected_index > 0 { selected_index -= 1; },
        Backspace => {
            match yomi.chars().last() { Some(c) if c.is_ascii() => { yomi.pop(); }, _ => (), }
            return InputState::Kana{ romaji: String::new(), state: KanaState::ToBeConverted(yomi) };
        },
        CommitCandidate => return commit_candidate(yomi, candidates, selected_index, buffer, jisyo),
        CommitCandidateWithChar(next) => {
            let next_state = commit_candidate(yomi, candidates, selected_index, buffer, jisyo);
            return handle_key(next_state, buffer, jisyo, Char(next));
        },
        CommitCandidateWithStartYomi(next) => {
            let next_state = commit_candidate(yomi, candidates, selected_index, buffer, jisyo);
            return handle_key(next_state, buffer, jisyo, StartYomi(next));
        },
        CommitCandidateWithSetsubiji => {
            let next_state = commit_candidate(yomi, candidates, selected_index, buffer, jisyo);
            return handle_key(next_state, buffer, jisyo, SetsubijiSettouji);
        },
        ToggleKatakana => {
            let next_state = commit_candidate(yomi, candidates, selected_index, buffer, jisyo);
            return handle_key(next_state, buffer, jisyo, ToggleKatakana);
        },
        _ => (),
    }
    InputState::Converting { yomi, candidates, selected_index }
}

// -------------------- Helpers --------------------

fn commit_candidate(
    mut yomi: String,
    candidates: Vec<String>,
    selected_index: usize,
    buffer: &mut Buffer,
    jisyo: &Jisyo,
) -> InputState {
    let cand = candidates.get(selected_index);
    let (commit, _) = crate::state::split_candidate(cand.unwrap());
    buffer.insert_str(commit);
    let mut next_state = InputState::new_kana();
    if let Some(okurigana) =yomi.pop() && okurigana.is_ascii_lowercase() {
        next_state = handle_key(next_state, buffer, jisyo, KeyEvent::Char(okurigana));
    }
    next_state
}

fn commit_kana(buffer: &mut Buffer, state: &mut KanaState, kana: &str) {
    use KanaState::{*};
    match state {
        ToBeConverted(yomi) => yomi.push_str(kana),
        Hiragana(_) => buffer.insert_str(kana),
        Katakana(hankaku) => buffer.insert_str( 
            &( if *hankaku { convert_to_halfwidth_katakana(kana)} else { convert_to_katakana(kana)} )
        ),
    }
}

fn convert_to_katakana(hiragana: &str) -> String {
    const OFFSET: u32 = 0x60;
    hiragana.chars()
        .map(|c| {
            if (0x3041..=0x3096).contains(&(c as u32)) { char::from_u32(c as u32 + OFFSET).unwrap() } else { c }
        })
        .collect()
}

pub fn convert_to_halfwidth_katakana(hiragana: &str) -> String {
    let mut result = String::with_capacity(hiragana.len());
    for c in hiragana.chars() {
        match HIRAGANA_TO_HALFWIDTH_KATAKANA.binary_search_by_key(&c, |&(k, _)| k) {
            Ok(idx) => result.push_str(HIRAGANA_TO_HALFWIDTH_KATAKANA[idx].1),
            Err(_) => result.push(c),
        }
    }
    result
} 

fn convert_to_zenkaku_ascii(c: char) -> char {
    match c { '!'..='~' => char::from_u32(c as u32 + 0xFEE0).unwrap(), ' ' => '　', _ => c }
}
