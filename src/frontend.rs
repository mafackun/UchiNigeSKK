use std::{
    io::{self, Read, Write},
    process::{Command, Stdio},
};

use termion::{event::Key, input::TermRead};

use crate::{
    buffer::Buffer,
    engine::handle_key,
    jisyo::Jisyo,
    key::{KeyEvent, Move},
    state::{InputState, KanaState},
    util::{
        ClosedInterval, push_char_to_vec_u8, push_itoa_usize_to_string, push_itoa_usize_to_vec_u8,
        push_str_to_vec_u8,
    },
};

struct CharWidth(u8);
const REPLACE: CharWidth = CharWidth(0);

// 線形探索向け：よく使う文字を先頭に
const SIMPLIFIED_WIDTH_TABLE: &[(ClosedInterval<u32>, CharWidth)] = &[
    // 幅2：日本語／CJK系
    (ClosedInterval(0x3040, 0x309F), CharWidth(2)), // Hiragana
    (ClosedInterval(0x30A0, 0x30FF), CharWidth(2)), // Katakana
    // 幅2：漢字系
    (ClosedInterval(0x2E80, 0xD7A3), CharWidth(2)), // CJK部首〜Hangul Syllables
    (ClosedInterval(0xF900, 0xFAFF), CharWidth(2)), // CJK Compatibility Ideographs
    (ClosedInterval(0x20000, 0x3FFFF), CharWidth(2)), // 拡張漢字
    // 幅1：半角カナ
    (ClosedInterval(0xFF61, 0xFF9F), CharWidth(1)),
    // 幅2：全角記号
    (ClosedInterval(0xFF01, 0xFF60), CharWidth(2)),
    (ClosedInterval(0xFFE0, 0xFFE6), CharWidth(2)),
    // 幅2：Hangul Jamo（あまり使わないので最後でもOK）
    (ClosedInterval(0x1100, 0x11FF), CharWidth(2)),

    // 制御文字（最も一般的）
    (ClosedInterval(0x00, 0x1F), REPLACE),
    (ClosedInterval(0x7F, 0x9F), REPLACE),
    // ZWSP / ZWNJ / ZWJ / LRM / RLM
    (ClosedInterval(0x200B, 0x200F), REPLACE),
    // Combining marks（入力やコピペで出ることがある）
    (ClosedInterval(0x0300, 0x036F), REPLACE),
    (ClosedInterval(0x1AB0, 0x1AFF), REPLACE),
    (ClosedInterval(0x1DC0, 0x1DFF), REPLACE),
    (ClosedInterval(0x20D0, 0x20FF), REPLACE),
    (ClosedInterval(0xFE20, 0xFE2F), REPLACE),
    // Variation Selector（EmojiなどのVS）
    (ClosedInterval(0xFE00, 0xFE0F), REPLACE),
    // Bidi制御文字
    (ClosedInterval(0x202A, 0x202E), REPLACE),
    (ClosedInterval(0x2066, 0x2069), REPLACE),
    // Emoji（コピーで来やすい）
    (ClosedInterval(0x1F300, 0x1FAFF), REPLACE),
    // IVS（異体字セレクタ）
    (ClosedInterval(0xE0100, 0xE01EF), REPLACE),
    // Tag characters（旗や絵文字用）
    (ClosedInterval(0xE0000, 0xE007F), REPLACE),
];

const DIM: &str = "\x1b[2m";
const CURSOR: &str = "\x1b[7m";
const RESET: &str = "\x1b[0m";
const STATUS: &str = "\x1b[97m\x1b[44m";
const CLEAR_ALL: &str = "\x1b[2J";
const CLEAR_CUR_LINE: &str = "\x1b[2K";
const CURSOR_SHOW: &str = "\x1b[?25h";
const CURSOR_HIDE: &str = "\x1b[?25l";

const SYMB_CHAR_W: usize = 1;
const SYMB_MORE_L: char = '<'; // 行省略記号(左)
const SYMB_MORE_R: char = '>'; // 行省略記号(右)
const SYMB_NO_LINE: char = '~';
const SYMB_LF: char = '¶';

const REPLACED_CHAR_W: usize = 2;
const REPLACE_TAB: &str = "\\t";
const REPLACE_OTHER: &str = "\\?";

const SCROLL_MARGIN: usize = 8; // 横スクロール開始の余裕幅(半角); ViewStateのサンプリングを考慮する
const CURSOR_SAMPLING_MASK: usize = 0b11;
const MIN_TERM_H: usize = 2;

// -------------------- キーバインド --------------------
enum FrontCmd {
    SendAndClear,
    Quit,
    Paste,
    Undo,
    Clear,
    Refresh,
    CopySelected,
    CutSelected,
    PrintCodePoint,
}

fn to_front_cmd(k: &Key) -> Option<FrontCmd> {
    use termion::event::Key::*;
    match k {
        Ctrl('q') => Some(FrontCmd::Quit),
        Ctrl('s') => Some(FrontCmd::SendAndClear),
        Ctrl('d') => Some(FrontCmd::Clear),
        Ctrl('r') => Some(FrontCmd::Refresh),
        Ctrl('x') => Some(FrontCmd::CutSelected),
        Ctrl('v') => Some(FrontCmd::Paste),
        Ctrl('c') => Some(FrontCmd::CopySelected),
        Ctrl('b') => Some(FrontCmd::PrintCodePoint),
        Esc => Some(FrontCmd::Undo),
        _ => None,
    }
}

fn to_key_event_global(k: &Key) -> Option<KeyEvent> {
    use termion::event::Key::*;
    match k {
        Ctrl('z') => Some(KeyEvent::ToggleHankakuZenkaku),
        Ctrl('l') => Some(KeyEvent::ToggleLatin),
        Ctrl('g') => Some(KeyEvent::CancelConversion),
        Left => Some(KeyEvent::Navigation(Move::Left)),
        Right => Some(KeyEvent::Navigation(Move::Right)),
        Up => Some(KeyEvent::Navigation(Move::Up)),
        Down => Some(KeyEvent::Navigation(Move::Down)),
        Home => Some(KeyEvent::Navigation(Move::LineHead)),
        End => Some(KeyEvent::Navigation(Move::LineTail)),
        PageUp => Some(KeyEvent::Navigation(Move::RapidUp)),
        PageDown => Some(KeyEvent::Navigation(Move::RapidDown)),
        ShiftLeft => Some(KeyEvent::Navigation(Move::SelectLeft)),
        ShiftRight => Some(KeyEvent::Navigation(Move::SelectRight)),
        Delete => Some(KeyEvent::Delete),
        Backspace => Some(KeyEvent::Backspace),
        _ => None,
    }
}

fn to_key_event_latin(k: &Key) -> Option<KeyEvent> {
    use termion::event::Key::*;
    match k {
        Char(c) => Some(KeyEvent::Char(*c)),
        _ => None,
    }
}

fn to_key_event_abbrev(k: &Key) -> Option<KeyEvent> {
    use termion::event::Key::*;
    match k {
        Char(' ') => Some(KeyEvent::StartConversion),
        Char('\n') => Some(KeyEvent::CommitUnconverted),
        Char(c) => Some(KeyEvent::Char(*c)),
        _ => None,
    }
}

fn to_key_event_kana(kana_state: &KanaState, k: &Key) -> Option<KeyEvent> {
    use termion::event::Key::*;
    match k {
        Char('q') => Some(KeyEvent::ToggleKatakana),
        Char('>') => Some(KeyEvent::Setsuji),
        Char('/') => Some(KeyEvent::StartAbbrev),
        Char(c @ ' ') => match kana_state {
            KanaState::ToBeConverted(_) => Some(KeyEvent::StartConversion),
            _ => Some(KeyEvent::Char(*c)),
        },
        Char(c @ '\n') => match kana_state {
            KanaState::ToBeConverted(_) => Some(KeyEvent::CommitUnconverted),
            _ => Some(KeyEvent::Char(*c)),
        },
        Char(c) if c.is_ascii_uppercase() => {
            Some(KeyEvent::StartYomiOrOkuri(c.to_ascii_lowercase()))
        }
        Char(c) => Some(KeyEvent::Char(*c)),
        _ => None,
    }
}

fn to_key_event_conversion(k: &Key) -> Option<KeyEvent> {
    use termion::event::Key::*;
    match k {
        Char(' ') => Some(KeyEvent::NextCandidate),
        Char('q') => Some(KeyEvent::ToggleKatakana),
        Char('x') => Some(KeyEvent::PrevCandidate),
        Char('\n') => Some(KeyEvent::CommitCandidate),
        Char('>') => Some(KeyEvent::CommitCandidateWithSetsubiji),
        Char('/') => Some(KeyEvent::StartAbbrev),
        Char(c) if c.is_ascii_uppercase() => Some(KeyEvent::CommitCandidateWithStartYomi(
            c.to_ascii_lowercase(),
        )),
        Char(c) => Some(KeyEvent::CommitCandidateWithChar(*c)),
        _ => None,
    }
}

fn to_key_event_with_state(state: &InputState, k: &Key) -> Option<KeyEvent> {
    if let Some(s) = to_key_event_global(k) {
        Some(s)
    } else {
        match state {
            InputState::Latin(_) => to_key_event_latin(k),
            InputState::Converting { .. } => to_key_event_conversion(k),
            InputState::Kana { state: s, .. } => to_key_event_kana(s, k),
            InputState::Abbrev { .. } => to_key_event_abbrev(k),
        }
    }
}

// -------------------- 文字幅 --------------------
#[inline(always)]
fn char_width(c: char) -> Option<usize> {
    let v = c as u32;
    if ClosedInterval(0x20, 0x7E).contains(v) {
        return Some(1);
    }
    for (interval, width) in SIMPLIFIED_WIDTH_TABLE {
        if interval.contains(v) {
            let w = width.0;
            return if w == 0 { None } else { Some(w as usize) };
        }
    }
    Some(1)
}

// -------------------- Viewport (スクロール) --------------------
#[derive(Default, Clone)]
struct ViewState {
    term_w: usize,
    left_cells: usize,
    active_line: usize,
    cursor_col: usize,
    active_line_offset: usize,
    ignore_inactive_lines: bool,
}

impl ViewState {
    fn update(&mut self, buffer: &Buffer, term_w: usize) {
        let (r, c) = buffer.cursor();
        if r != self.active_line
            || c.abs_diff(self.cursor_col) > 1
            || Self::is_sampling_point(c)
            || term_w < self.term_w
        { 
            let line = buffer.line(r);
            let old_left_cells = self.left_cells;
            self.left_cells = Self::get_left_cells(old_left_cells, term_w, line, c);
            if r != self.active_line || self.left_cells != old_left_cells {
                self.active_line_offset = calc_offset(line, self.left_cells);
            }
        }
        self.active_line = r;
        self.cursor_col = c;
        self.term_w = term_w;
        self.ignore_inactive_lines = true;
    }

    #[inline(always)]
    fn is_sampling_point(c: usize) -> bool {
        c & CURSOR_SAMPLING_MASK == 0
    }

    #[inline(always)]
    fn should_redraw_all(&self, old: &Self) -> bool {
        self.left_cells != old.left_cells
            || self.active_line != old.active_line
            || self.ignore_inactive_lines != old.ignore_inactive_lines
    }

    fn get_left_cells(old_left_cells: usize, term_w: usize, line: &[char], cursor_col: usize) -> usize {
        let half_w = term_w / 2;
        let cur_cells: usize = line
            .iter()
            .take(cursor_col)
            .map(|c: &char| char_width(*c).unwrap_or(REPLACED_CHAR_W))
            .sum();

        let interval = ClosedInterval(
            old_left_cells + SCROLL_MARGIN,
            old_left_cells + term_w.saturating_sub(SCROLL_MARGIN),
        );

        if interval.contains(cur_cells) {
            old_left_cells
        } else {
            cur_cells.saturating_sub(half_w)
        }
    }
}

fn calc_offset(line: &[char], left_cells: usize) -> usize {
    let mut ignored_cells = 0usize;
    let mut offset = 0;
    for ch in line {
        let w = char_width(*ch).unwrap_or(REPLACED_CHAR_W);
        if ignored_cells + w > left_cells {
            break;
        }
        ignored_cells += w;
        offset += 1;
    }
    offset
}

// -------------------- prepare for drawing --------------------
enum SelectionState {
    Pre,
    In,
    Post,
}

fn prepare_view_to_buffer(
    out: &mut Vec<u8>,
    term_size: (usize, usize),
    vs: &mut ViewState,
    buffer: &Buffer,
) {
    let (term_w, term_h) = term_size;
    let (r, _) = buffer.cursor();
    let view_bottom = term_h - 1;
    let vs_old = vs.clone();
    vs.update(buffer, term_w);

    out.clear();
    for y in 1..=view_bottom {
        let active_line = y == view_bottom;
        if !vs.should_redraw_all(&vs_old) && !active_line {
            continue;
        }
        push_cursor_goto(out, y, 1);
        push_str_to_vec_u8(out, CLEAR_CUR_LINE);
        if let Some(row) = (r + y).checked_sub(view_bottom) {
            let raw_line = buffer.line(row);
            let sel = if active_line {
                Some(buffer.selection())
            } else {
                None
            };
            let lf = buffer.has_more_line(row);
            let i = if active_line {
                vs.active_line_offset
            } else {
                calc_offset(raw_line, vs.left_cells)
            };
            prepare_line_to_buffer(out, raw_line, i, term_w, sel, lf);
        } else {
            push_fmt_ch(out, DIM, SYMB_NO_LINE);
        }
    }
}

fn prepare_line_to_buffer(
    out: &mut Vec<u8>,
    line: &[char],
    offset: usize,
    term_w: usize,
    selection: Option<ClosedInterval<usize>>,
    lf: bool,
) {
    let mut used = 0usize;
    let mut ss = SelectionState::Pre;
    let mut end_of_line = true;
    for (i, c) in line.iter().enumerate().skip(offset) {
        let width_original = char_width(*c);
        let w = width_original.unwrap_or(REPLACED_CHAR_W);
        if used + w >= term_w {
            end_of_line = false;
            break;
        }

        // 左にオフセットなら行頭の1文字を潰してSYMB_MORE_Lを描画（見た目とセル数の安定性を優先）
        if i != 0 && used == 0 {
            push_fmt_ch(out, DIM, SYMB_MORE_L);
            used += SYMB_CHAR_W;
            continue;
        }

        let replace = width_original.is_none();
        let in_selection = matches!(selection, Some(ref interval) if interval.contains(i));
        handle_selection(out, &mut ss, in_selection);
        handle_push_character(out, *c, replace, in_selection);
        used += w;
    }

    if matches!(ss, SelectionState::In) {
        push_str_to_vec_u8(out, RESET);
    }

    if used == 0 && !line.is_empty() {
        push_fmt_ch(out, DIM, SYMB_MORE_L);
    } else if used < term_w {
        if end_of_line {
            // get_next_left_cells()が画面内にカーソルを配置することが前提
            let selection_remains = selection.is_some() && matches!(ss, SelectionState::Pre);
            let fmt = if selection_remains { CURSOR } else { DIM };
            let tail = if lf { SYMB_LF } else { ' ' };
            push_fmt_ch(out, fmt, tail);
        } else {
            push_fmt_ch(out, DIM, SYMB_MORE_R);
        }
    }
}

fn prepare_status_line(
    out: &mut Vec<u8>,
    term_size: (usize, usize),
    code_point: Option<&str>,
    state: &InputState,
    buffer: Option<&Buffer>,
    has_ss: bool,
) {
    let (term_w, term_h) = term_size;
    out.clear();

    push_cursor_goto(out, term_h, 1);
    push_str_to_vec_u8(out, STATUS);
    push_str_to_vec_u8(out, CLEAR_CUR_LINE);

    let mut usable_cells = term_w;
    if let Some(cp) = code_point {
        push_str_until(out, cp, &mut usable_cells);
        if usable_cells > 0 {
            push_char_to_vec_u8(out, ' ');
            usable_cells -= 1;
        }
    }
    push_str_until(out, &state.status_as_string(), &mut usable_cells);
    if let Some(b) = buffer {
        if usable_cells > 0 {
            push_char_to_vec_u8(out, ' ');
            usable_cells -= 1;
        }
        push_str_until(out, &b.status_as_string(), &mut usable_cells);
    }
    if has_ss {
        push_str_until(out, " +undo", &mut usable_cells);
    }

    push_str_to_vec_u8(out, RESET);
}

#[inline(always)]
fn handle_selection(out: &mut Vec<u8>, ss: &mut SelectionState, in_selection: bool) {
    if in_selection && matches!(ss, SelectionState::Pre) {
        *ss = SelectionState::In;
        push_str_to_vec_u8(out, CURSOR);
    } else if !in_selection && matches!(ss, SelectionState::In) {
        push_str_to_vec_u8(out, RESET);
        *ss = SelectionState::Post;
    }
}

#[inline(always)]
fn handle_push_character(out: &mut Vec<u8>, c: char, replace: bool, in_selection: bool) {
    let dim_replaced_char = replace && !in_selection;
    if dim_replaced_char {
        push_str_to_vec_u8(out, DIM);
    }
    push_replaced_char(out, c, replace);
    if dim_replaced_char {
        push_str_to_vec_u8(out, RESET);
    }
}

#[inline(always)]
fn push_fmt_ch(out: &mut Vec<u8>, fmt: &str, c: char) {
    push_str_to_vec_u8(out, fmt);
    push_char_to_vec_u8(out, c);
    push_str_to_vec_u8(out, RESET);
}

#[inline(always)]
fn push_replaced_char(out: &mut Vec<u8>, c: char, replace: bool) {
    if replace {
        let replaced = match c {
            '\t' => REPLACE_TAB,
            _ => REPLACE_OTHER,
        };
        push_str_to_vec_u8(out, replaced);
    } else {
        push_char_to_vec_u8(out, c);
    }
}

#[inline(always)]
pub fn push_cursor_goto(out: &mut Vec<u8>, row: usize, col: usize) {
    push_str_to_vec_u8(out, "\x1b[");
    push_itoa_usize_to_vec_u8(out, row, 10);
    push_char_to_vec_u8(out, ';');
    push_itoa_usize_to_vec_u8(out, col, 10);
    push_char_to_vec_u8(out, 'H');
}

pub fn push_str_until(out: &mut Vec<u8>, s: &str, cell_counter: &mut usize) {
    if *cell_counter == 0 {
        return;
    }
    for c in s.chars() {
        let width_original = char_width(c);
        let w = width_original.unwrap_or(REPLACED_CHAR_W);
        if (*cell_counter).saturating_sub(w) < 1 {
            break;
        }
        push_replaced_char(out, c, width_original.is_none());
        *cell_counter -= w
    }
}

// -------------------- terminal size --------------------
fn get_terminal_size() -> (usize, usize) {
    let (w, h) = termion::terminal_size().expect("failed to query terminal size");
    (w as usize, h as usize)
}

fn is_terminal_too_small(term_size: (usize, usize)) -> bool {
    // 否定がredrawの前提
    let (term_w, term_h) = term_size;
    term_w < SCROLL_MARGIN * 2 + 20 || term_h < MIN_TERM_H
}

// -------------------- drawing --------------------
fn draw_terminal_too_small<W: Write>(out: &mut W) -> io::Result<()> {
    let mut buf: Vec<u8> = Vec::new();
    push_cursor_goto(&mut buf, 1, 1);
    push_str_to_vec_u8(&mut buf, CLEAR_ALL);
    push_str_to_vec_u8(&mut buf, "RESIZE_AND_REFRESH");
    out.write_all(&buf)?;
    out.flush()?;
    Ok(())
}

fn redraw<W: Write>(
    out: &mut W,
    view: Option<&[u8]>,
    status_line: Option<&[u8]>,
) -> io::Result<()> {
    if let Some(v) = view {
        out.write_all(v)?;
    }
    if let Some(sl) = status_line {
        out.write_all(sl)?;
    }
    out.flush()?;
    Ok(())
}

// -------------------- command --------------------
fn copy_to_command(text: &str, shell: &str, cmd: &str) {
    let mut child = Command::new(shell)
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::piped())
        .spawn()
        .expect("command CPY_TO failure");
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .expect("failed to convert text into bytes");
    }
    let _ = child.wait();
}

fn copy_from_command(shell: &str, cmd: &str) -> String {
    let out = Command::new(shell)
        .arg("-c")
        .arg(cmd)
        .output()
        .expect("command CPY_FROM failure");
    String::from_utf8_lossy(&out.stdout).to_string()
}

// -------------------- snapshot --------------------
fn take_snapshot(has_ss: &mut bool, buffer: &Buffer, ss: &mut Buffer) {
    *ss = buffer.clone();
    *has_ss = true;
}

fn drop_snapshot(has_ss: &mut bool, ss: &mut Buffer) {
    ss.clear();
    *has_ss = false;
}

// -------------------- public --------------------
pub fn cleanup<W: Write>(out: &mut W) -> io::Result<()> {
    let mut buf: Vec<u8> = Vec::new();
    push_cursor_goto(&mut buf, 1, 1);
    push_str_to_vec_u8(&mut buf, CLEAR_ALL);
    push_str_to_vec_u8(&mut buf, CURSOR_SHOW);
    out.write_all(&buf)?;
    out.flush()
}

pub fn run<W, R>(
    mut ui: W,
    input: R,
    jisyo: Jisyo,
    shell: &str,
    cpyt: &str,
    cpyf: &str,
) -> io::Result<()>
where
    W: Write,
    R: Read,
{
    let mut b = Buffer::default();
    let mut ss = Buffer::default();
    let mut is = InputState::new_kana();
    let mut vs = ViewState::default();
    let mut has_ss = false;

    ui.write_all(CURSOR_HIDE.as_bytes())?;
    ui.flush()?;

    let mut ts = get_terminal_size();
    let mut too_small = is_terminal_too_small(ts);
    let mut sl: Vec<u8> = Vec::new();
    let mut v: Vec<u8> = Vec::new();
    if !too_small {
        prepare_view_to_buffer(&mut v, ts, &mut vs, &b);
        prepare_status_line(&mut sl, ts, None, &is, None, has_ss);
        redraw(&mut ui, Some(&v), Some(&sl))?;
    } else {
        draw_terminal_too_small(&mut ui)?;
    }

    for key in input.keys() {
        let k = match key {
            Ok(k) => k,
            Err(_) => continue,
        };
        if let Some(cmd) = to_front_cmd(&k) {
            match cmd {
                FrontCmd::Quit => break,
                FrontCmd::Refresh => {
                    ts = get_terminal_size();
                    too_small = is_terminal_too_small(ts);
                    if too_small {
                        draw_terminal_too_small(&mut ui)?;
                        continue;
                    }
                    vs.ignore_inactive_lines = false;
                    prepare_view_to_buffer(&mut v, ts, &mut vs, &b);
                    prepare_status_line(&mut sl, ts, None, &is, Some(&b), has_ss);
                    redraw(&mut ui, Some(&v), Some(&sl))?;
                    ui.write_all(CURSOR_HIDE.as_bytes())?;
                }

                _commands_below if too_small => { /* do nothing */ },
                FrontCmd::Clear => {
                    take_snapshot(&mut has_ss, &b, &mut ss);
                    b.clear();
                    prepare_view_to_buffer(&mut v, ts, &mut vs, &b);
                    prepare_status_line(&mut sl, ts, None, &is, None, has_ss);
                    redraw(&mut ui, Some(&v), Some(&sl))?;
                }
                FrontCmd::SendAndClear => {
                    take_snapshot(&mut has_ss, &b, &mut ss);
                    copy_to_command(&b.as_string(), shell, cpyt);
                    b.clear();
                    prepare_view_to_buffer(&mut v, ts, &mut vs, &b);
                    prepare_status_line(&mut sl, ts, None, &is, None, has_ss);
                    redraw(&mut ui, Some(&v), Some(&sl))?;
                }
                FrontCmd::Paste => {
                    take_snapshot(&mut has_ss, &b, &mut ss);
                    b.insert_str(&copy_from_command(shell, cpyf));
                    prepare_view_to_buffer(&mut v, ts, &mut vs, &b);
                    prepare_status_line(&mut sl, ts, None, &is, Some(&b), has_ss);
                    redraw(&mut ui, Some(&v), Some(&sl))?;
                }
                FrontCmd::CopySelected => {
                    if let Some(s) = b.selected_as_string() {
                        copy_to_command(&s, shell, cpyt);
                    }
                }
                FrontCmd::CutSelected => {
                    if let Some(s) = b.selected_as_string() {
                        take_snapshot(&mut has_ss, &b, &mut ss);
                        copy_to_command(&s, shell, cpyt);
                        b.delete();
                        prepare_view_to_buffer(&mut v, ts, &mut vs, &b);
                        prepare_status_line(&mut sl, ts, None, &is, Some(&b), has_ss);
                        redraw(&mut ui, Some(&v), Some(&sl))?;
                    }
                }
                FrontCmd::PrintCodePoint => {
                    if let Some(c) = b.cursor_as_char() {
                        let mut cp = String::from("[U+");
                        push_itoa_usize_to_string(&mut cp, *c as usize, 16);
                        cp.push(']');
                        prepare_status_line(&mut sl, ts, Some(&cp), &is, Some(&b), has_ss);
                        redraw(&mut ui, None, Some(&sl))?;
                    }
                }
                FrontCmd::Undo => {
                    if !has_ss {
                        continue;
                    }
                    (b, ss) = (ss, b);
                    prepare_view_to_buffer(&mut v, ts, &mut vs, &b);
                    prepare_status_line(&mut sl, ts, None, &is, Some(&b), has_ss);
                    redraw(&mut ui, Some(&v), Some(&sl))?;
                }
            }
        }
        if let Some(ev) = to_key_event_with_state(&is, &k)
            && !too_small
        {
            b.clear_dirty();
            is = handle_key(is, &mut b, &jisyo, ev);
            let view: Option<&[u8]> = if b.is_dirty() {
                prepare_view_to_buffer(&mut v, ts, &mut vs, &b);
                Some(&v)
            } else {
                None
            };
            if let KeyEvent::Navigation(_) = ev {
                prepare_status_line(&mut sl, ts, None, &is, Some(&b), has_ss);
            } else {
                drop_snapshot(&mut has_ss, &mut ss);
                prepare_status_line(&mut sl, ts, None, &is, None, has_ss);
            };
            redraw(&mut ui, view, Some(&sl))?;
        }
    }

    cleanup(&mut ui)
}
