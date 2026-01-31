use std::{
    fs::OpenOptions,
    io::{self, Write},
    process::{Command, Stdio},
};

use termion::{
    clear, cursor, event::Key, input::TermRead, raw::IntoRawMode, screen::IntoAlternateScreen,
};

use crate::{
    buffer::Buffer,
    engine::handle_key,
    jisyo::Jisyo,
    key::KeyEvent,
    state::{InputState, KanaState},
};

const SIMPLIFIED_WIDTH_TABLE: &[(std::ops::RangeInclusive<u32>, usize)] = &[
    (0..=0x1F, 4),        // control
    (0x7F..=0x7F, 4),     // delete
    (0x0300..=0x036F, 0), // zero-width
    (0x200B..=0x200F, 0), // zero-width
    (0x20..=0x7E, 1),     // ascii
    (0xFF61..=0xFF9F, 1), // hanakaku katakana
    (0x00A0..=0x00FF, 1), // latin
    (0x0100..=0x1FFF, 1), // latin/europe
];

enum FrontCmd {
    SendAndClear,
    Quit,
    Paste,
    Undo,
    Clear,
    Refresh,
}

fn to_front_cmd(k: &Key) -> Option<FrontCmd> {
    use termion::event::Key::*;
    match k {
        Ctrl('q') => Some(FrontCmd::Quit),
        Ctrl('v') => Some(FrontCmd::Paste),
        Ctrl('s') => Some(FrontCmd::SendAndClear),
        Ctrl('d') => Some(FrontCmd::Clear),
        Ctrl('r') => Some(FrontCmd::Refresh),
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
        Left => Some(KeyEvent::MoveLeft),
        Right => Some(KeyEvent::MoveRight),
        Up => Some(KeyEvent::MoveUp),
        Down => Some(KeyEvent::MoveDown),
        Home => Some(KeyEvent::ToLineHead),
        End => Some(KeyEvent::ToLineTail),
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
        Char(c) if c.is_ascii_uppercase() => Some(KeyEvent::CommitCandidateWithStartYomi(
            c.to_ascii_lowercase(),
        )),
        Char(c) => Some(KeyEvent::CommitCandidateWithChar(*c)),
        _ => None,
    }
}

fn to_key_event_with_state(state: &InputState, k: &Key) -> Option<KeyEvent> {
    if let Some(s) = to_key_event_global(k) {
        return Some(s);
    }
    match state {
        InputState::Latin(_) => to_key_event_latin(k),
        InputState::Converting { .. } => to_key_event_conversion(k),
        InputState::Kana { state: s, .. } => to_key_event_kana(s, k),
        InputState::Abbrev { .. } => to_key_event_abbrev(k),
    }
}

// -------------------- Viewport (縦・横スクロール) --------------------
#[derive(Debug, Clone, Copy)]
struct Viewport {
    top_row: usize,
    left_cells: usize,
    status_rows: usize,
    scroll_margin: usize,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            top_row: 0,
            left_cells: 0,
            status_rows: 2,
            scroll_margin: 4,
        }
    }
}

fn char_width(c: char) -> usize {
    for (range, width) in SIMPLIFIED_WIDTH_TABLE {
        if range.contains(&(c as u32)) {
            return *width;
        }
    }
    2
}

fn cells_up_to(chars: &[char], col_chars: usize) -> usize {
    chars.iter().take(col_chars).map(|&c| char_width(c)).sum()
}

fn snap_left_cells_to_boundary(chars: &[char], desired_cells: usize) -> usize {
    let mut cells = 0usize;
    let mut last_boundary = 0usize;

    for &c in chars {
        let w = char_width(c);
        if cells + w > desired_cells {
            break;
        }
        cells += w;
        last_boundary = cells;
    }
    last_boundary
}

fn format_line_for_print(chars: &[char], left_cells: usize, width_cells: usize) -> String {
    use std::fmt::Write;
    const FMT: &str = "\x1b[2m";
    const RES: &str = "\x1b[0m";
    const MORE_L: char = '<';
    const MORE_R: char = '>';
    let mut ignored_cells = 0usize;
    let mut start_idx = 0;
    for &ch in chars.iter() {
        let w = char_width(ch);
        if ignored_cells + w > left_cells {
            break;
        }
        ignored_cells += w;
        start_idx += 1;
    }

    let mut out = String::new();
    let mut used = 0usize;
    for (i, &ch) in chars.iter().enumerate().skip(start_idx) {
        let w = char_width(ch);
        if used + w >= width_cells {
            _ = write!(out, "{FMT}{}{RES}", MORE_R);
            break;
        }
        if i != 0 && used == 0 {
            _ = write!(out, "{FMT}{}{RES}", MORE_L);
            for _ in 0..(w.saturating_sub(1)) {
                out.push(' ');
            } // カーソルのズレ防止に文字幅をスペースで調整 
        } else {
            match chars[i] as u32 {
                0x09 => _ = write!(out, "{FMT}\\TAB{RES}"),
                v @ 0x00..0x20 | v @ 0x7F => _ = write!(out, "{FMT}\\x{:02x}{RES}", v),
                _ => out.push(ch),
            }
        }
        used += w;
    }

    // 非空の行が画面外なら記号
    if out.is_empty() && !chars.is_empty() {
        _ = write!(out, "{FMT}{}{RES}", MORE_L);
    }
    out
}

fn clamp_viewport_to_cursor_y(vp: &mut Viewport, buffer: &Buffer, view_h: usize) {
    let (cur_row, _) = buffer.cursor();

    if view_h == 0 {
        vp.top_row = cur_row;
        return;
    }
    if cur_row < vp.top_row {
        vp.top_row = cur_row;
        return;
    }

    let bottom = vp.top_row + view_h - 1;
    if cur_row > bottom {
        vp.top_row = cur_row - (view_h - 1);
    }
}

fn clamp_viewport_to_cursor_x(vp: &mut Viewport, buffer: &Buffer, view_w: usize) {
    if view_w == 0 {
        return;
    }

    let (r, c) = buffer.cursor();
    let line = buffer.line_as_string(r);
    let chars: Vec<char> = line.chars().collect();

    let cur_cells = cells_up_to(&chars, c);

    // マージンを画面幅に対して安全側に丸める
    // (小さすぎる端末で margin が大きすぎると破綻するので)
    let m = vp.scroll_margin.min(view_w.saturating_sub(1));
    let left_bound = vp.left_cells + m;

    // 右側は「表示幅 - 1 - margin」を境界にする
    // view_w=80, m=3 なら right_bound = left+76
    let right_bound = vp.left_cells + view_w.saturating_sub(1 + m);

    // 左側に寄りすぎたら左へスクロール
    if cur_cells < left_bound {
        let desired_left = cur_cells.saturating_sub(m);
        vp.left_cells = snap_left_cells_to_boundary(&chars, desired_left);
        return;
    }

    // 右側に寄りすぎたら右へスクロール
    if cur_cells > right_bound {
        let desired_left = cur_cells.saturating_sub(view_w.saturating_sub(1 + m));
        vp.left_cells = snap_left_cells_to_boundary(&chars, desired_left);
        return;
    }

    // 念のため境界にスナップ（幅2文字の途中セルを避ける）
    vp.left_cells = snap_left_cells_to_boundary(&chars, vp.left_cells);
}

// -------------------- redraw --------------------

fn redraw<W: Write>(
    out: &mut W,
    buffer: &Buffer,
    state: &InputState,
    vp: &mut Viewport,
    has_snap: bool,
) {
    let (term_w, term_h) = {
        let (w, h) = termion::terminal_size().unwrap_or((80, 24));
        (w as usize, h as usize)
    };
    let status_rows = vp.status_rows.max(1);
    let view_h = term_h.saturating_sub(status_rows).max(1);

    clamp_viewport_to_cursor_y(vp, buffer, view_h);
    clamp_viewport_to_cursor_x(vp, buffer, term_w);

    let total_lines = buffer.line_count().max(1);

    let _ = write!(out, "{}{}", clear::All, cursor::Goto(1, 1));

    // ---------- Buffer viewport ----------
    for screen_y in 0..view_h {
        let y = (screen_y + 1) as u16;
        let row = vp.top_row + screen_y;

        let shown = if row < total_lines {
            let line = buffer.line_as_string(row);
            let chars: Vec<char> = line.chars().collect();
            format_line_for_print(&chars, vp.left_cells, term_w)
        } else {
            String::new()
        };

        let _ = write!(out, "{}{}{}", cursor::Goto(1, y), clear::CurrentLine, shown);
    }

    // ---------- Status ----------
    // status 先頭行（1-based）
    let status_top_y = view_h + 1;

    // status 領域を消してから書く（行数固定）
    for i in 0..status_rows {
        let y = (status_top_y + i) as u16;
        let _ = write!(out, "{}{}", cursor::Goto(1, y), clear::CurrentLine);
    }

    let (state_shown, buffer_shown) = {
        let s: Vec<char> = format!("┌ {}", state).chars().collect();
        let b: Vec<char> = format!("└ {}{}", buffer, if has_snap { " +undo" } else { "" })
            .chars()
            .collect();
        (
            format_line_for_print(&s, 0, term_w),
            format_line_for_print(&b, 0, term_w),
        )
    };

    let _ = write!(
        out,
        "{}{}",
        cursor::Goto(1, status_top_y as u16),
        state_shown
    );
    let _ = write!(
        out,
        "{}{}",
        cursor::Goto(1, (status_top_y + status_rows) as u16),
        buffer_shown
    );

    // ---------- Cursor ----------
    let (row, col_chars) = buffer.cursor();

    if row >= vp.top_row && row < vp.top_row + view_h {
        let cur_line = buffer.line_as_string(row);
        let chars: Vec<char> = cur_line.chars().collect();

        let cur_cells = cells_up_to(&chars, col_chars);
        let x_cells = cur_cells.saturating_sub(vp.left_cells);

        // 1-based. 端末幅に収める（termion は term_w+1 を許容しない）
        let screen_x = (x_cells + 1).clamp(1, term_w.max(1)) as u16;

        // 本文の開始Yは 1 行目。status は view_h+1 から始める仕様
        let screen_y = (1 + (row - vp.top_row)) as u16;

        let _ = write!(out, "{}", cursor::Goto(screen_x, screen_y));
    } else {
        // 念のため（通常ここには来ない）
        let _ = write!(out, "{}", cursor::Goto(1, 1));
    }

    let _ = out.flush();
}

// -------------------- clipboard --------------------

fn split_cmd_args(full_cmd: &str) -> (&str, &str) {
    let mut split = full_cmd.splitn(2, ' ');
    (
        split.next().expect("invalid environment variable"),
        split.next().unwrap_or_default(),
    )
}

fn copy_to_clipboard(text: &str, cpyt: &str) {
    let (cmd, args) = split_cmd_args(cpyt);
    let mut child = Command::new(cmd)
        .args(args.split_whitespace())
        .stdin(Stdio::piped())
        .spawn()
        .expect("command CPY_TO failure");
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).unwrap();
    }
    let _ = child.wait();
}

fn copy_from_clipboard(cpyf: &str) -> String {
    let (cmd, args) = split_cmd_args(cpyf);
    let out = Command::new(cmd)
        .args(args.split_whitespace())
        .output()
        .expect("command CPY_FROM failure");
    String::from_utf8_lossy(&out.stdout).to_string()
}

// -------------------- snapshot --------------------
fn take_snapshot(is_take: bool, has_snap: &mut bool, buffer: &Buffer, snap: &mut Buffer) {
    if is_take {
        *snap = buffer.clone();
    } else {
        snap.clear();
    }
    *has_snap = is_take;
}

// -------------------- run --------------------
pub fn run(jisyo: Jisyo, cpyt: &str, cpyf: &str) -> io::Result<()> {
    let tty_in = OpenOptions::new().read(true).open("/dev/tty")?;
    let mut ui = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")?
        .into_raw_mode()?
        .into_alternate_screen()?;

    let (mut buffer, mut snap) = (Buffer::default(), Buffer::default());
    let mut state = InputState::new_kana();
    let mut vp = Viewport::default();
    let mut has_snap = false;

    let _ = write!(ui, "{}", cursor::Show);
    let _ = ui.flush();

    redraw(&mut ui, &buffer, &state, &mut vp, has_snap);

    for key in tty_in.keys() {
        let k = match key {
            Ok(k) => k,
            Err(_) => continue,
        };

        if let Some(cmd) = to_front_cmd(&k) {
            match cmd {
                FrontCmd::Quit => break,
                FrontCmd::Refresh => {
                    redraw(&mut ui, &buffer, &state, &mut vp, has_snap);
                    continue;
                }
                FrontCmd::Clear => {
                    take_snapshot(true, &mut has_snap, &buffer, &mut snap);
                    buffer.clear();
                    redraw(&mut ui, &buffer, &state, &mut vp, has_snap);
                    continue;
                }
                FrontCmd::SendAndClear => {
                    take_snapshot(true, &mut has_snap, &buffer, &mut snap);
                    copy_to_clipboard(&buffer.as_string(), cpyt);
                    buffer.clear();
                    redraw(&mut ui, &buffer, &state, &mut vp, has_snap);
                    continue;
                }
                FrontCmd::Paste => {
                    take_snapshot(true, &mut has_snap, &buffer, &mut snap);
                    buffer.insert_str(&copy_from_clipboard(cpyf));
                    redraw(&mut ui, &buffer, &state, &mut vp, has_snap);
                    continue;
                }
                FrontCmd::Undo => {
                    if !has_snap {
                        continue;
                    }
                    (buffer, snap) = (snap, buffer);
                    redraw(&mut ui, &buffer, &state, &mut vp, has_snap);
                    continue;
                }
            }
        }

        if let Some(ev) = to_key_event_with_state(&state, &k) {
            take_snapshot(false, &mut has_snap, &buffer, &mut snap);
            state = handle_key(state, &mut buffer, &jisyo, ev);
            redraw(&mut ui, &buffer, &state, &mut vp, has_snap);
        }
    }

    let _ = write!(ui, "{}{}{}", clear::All, cursor::Goto(1, 1), cursor::Show);
    let _ = ui.flush();
    Ok(())
}
