use std::{
    fs::OpenOptions,
    io::{self, Write},
    process::{Command, Stdio},
};

use termion::{
    clear,
    cursor,
    event::Key,
    input::TermRead,
    raw::IntoRawMode,
    screen::IntoAlternateScreen,
};

use crate::{
    buffer::Buffer,
    engine::handle_key,
    key::KeyEvent,
    state::{InputState, KanaState},
    jisyo::Jisyo,
};

enum FrontCmd {
    SendAndClear,
    Quit,
    Paste,
    Prev,
    Clear,
}

fn to_front_cmd(k: &Key) -> Option<FrontCmd> {
    match k {
        Key::Ctrl('c') => Some(FrontCmd::Quit),
        Key::Ctrl('v') => Some(FrontCmd::Paste),
        Key::Ctrl('s') => Some(FrontCmd::SendAndClear),
        Key::Ctrl('u') => Some(FrontCmd::Prev),
        Key::Ctrl('d') => Some(FrontCmd::Clear),
        _ => None,
    }
}

fn to_key_event_with_state(state: &InputState, k: &Key) -> Option<KeyEvent> {
    // 1) グローバル（状態に依存しない意図）
    match k {
        Key::Ctrl('z') => return Some(KeyEvent::ToggleHankakuZenkaku),
        Key::Ctrl('l') => return Some(KeyEvent::ToggleLatin),
        Key::Ctrl('q') => return Some(KeyEvent::ToggleKatakana),
        Key::Alt('`') => return Some(KeyEvent::ToggleHKL),
        Key::Esc => return Some(KeyEvent::CancelConversion),

        Key::Left => return Some(KeyEvent::MoveLeft),
        Key::Right => return Some(KeyEvent::MoveRight),
        Key::Up => return Some(KeyEvent::MoveUp),
        Key::Down => return Some(KeyEvent::MoveDown),
        Key::Home => return Some(KeyEvent::ToLineHead),
        Key::End => return Some(KeyEvent::ToLineTail),
        Key::Delete => return Some(KeyEvent::Delete),

        _ => {}
    }

    // 2) 状態ごとの解釈
    match state {
        InputState::Latin(_) => match k {
            Key::Backspace => Some(KeyEvent::Backspace),
            Key::Char(c) => Some(KeyEvent::Char(*c)),
            _ => None,
        },

        InputState::Kana { state: kana_state, .. } => match k {
            Key::Backspace => Some(KeyEvent::Backspace),
            Key::Char(' ') => match kana_state {
                KanaState::ToBeConverted(_) => Some(KeyEvent::StartConversion),
                _ => Some(KeyEvent::Char(' ')),
            },
            Key::Char('\n') => match kana_state {
                KanaState::ToBeConverted(_) => Some(KeyEvent::CommitUnconvertedYomi),
                _ => Some(KeyEvent::Char('\n')),
            },
            Key::Char('>') => Some(KeyEvent::SetsubijiSettouji),
            Key::Char(c) if c.is_ascii_uppercase() => match kana_state {
                KanaState::ToBeConverted(_) => Some(KeyEvent::Okurigana(c.to_ascii_lowercase())),
                _ => Some(KeyEvent::StartYomi(c.to_ascii_lowercase())),
            },
            Key::Char(c) => Some(KeyEvent::Char(*c)),
            _ => None,
        },

        InputState::Converting { .. } => match k {
            Key::Char(' ') => Some(KeyEvent::NextCandidate),
            Key::Char('x') => Some(KeyEvent::PrevCandidate),
            Key::Char('\n') => Some(KeyEvent::CommitCandidate),
            Key::Char('>') => Some(KeyEvent::CommitCandidateWithSetsubiji),
            Key::Char(c) if c.is_ascii_uppercase() => 
                Some(KeyEvent::CommitCandidateWithStartYomi(c.to_ascii_lowercase())),
            Key::Char(c) => Some(KeyEvent::CommitCandidateWithChar(*c)),
            Key::Backspace => Some(KeyEvent::Backspace),
            _ => None,
        },
    }
}

// -------------------- Viewport (縦・横スクロール) --------------------
#[derive(Debug, Clone, Copy)]
struct Viewport {
    top_row: usize,
    left_cells: usize,
    status_rows: usize,
    scroll_margin: usize, // 追加: 端から何セルでスクロール開始するか
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            top_row: 0,
            left_cells: 0,
            status_rows: 2,
            scroll_margin: 3, // 追加: 好みで調整
        }
    }
}

fn char_width(c: char) -> usize {
    match c {
        '\x00'..='\x7F' | '｡'..='ﾟ' => 1,
        _ => 2,
    }
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

fn slice_line_by_cells(chars: &[char], left_cells: usize, width_cells: usize) -> String {
    let mut cells = 0usize;
    let mut start = 0usize;
    while start < chars.len() {
        let w = char_width(chars[start]);
        if cells + w > left_cells {
            break;
        }
        cells += w;
        start += 1;
    }

    let mut out = String::new();
    let mut used = 0usize;
    let mut i = start;
    while i < chars.len() {
        let w = char_width(chars[i]);
        if used + w > width_cells {
            break;
        }
        out.push(chars[i]);
        used += w;
        i += 1;
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

fn redraw<W: Write>(out: &mut W, buffer: &Buffer, state: &InputState, vp: &mut Viewport) {
    let (w_u16, h_u16) = termion::terminal_size().unwrap_or((80, 24));
    let term_w = w_u16 as usize;
    let term_h = h_u16 as usize;

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
           let chars: Vec<char> = line
                .chars()
                .map( |c| match c as u32 { 
                    v@0x00..=0x1F => char::from_u32(0x2400+v).unwrap(), 0x7F=>'␡', _=>c
                } )
                .collect();
            slice_line_by_cells(&chars, vp.left_cells, term_w)
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

    let _ = write!(out, "{}┌ {}", cursor::Goto(1, status_top_y as u16), state);
    let _ = write!(out, "{}└ {}", cursor::Goto(1, (status_top_y + status_rows - 1) as u16), buffer);

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
    (split.next().expect("invalid environment variable"), if let Some(s) = split.next() {s} else {""})
}

fn copy_to_clipboard(text: &str, cpyt: &str) {
    let (cmd, args) = split_cmd_args(cpyt);
    let mut child = Command::new(cmd)
        .args(args.split_whitespace())
        .stdin(Stdio::piped())
        .spawn()
        .expect("command CPY_TO failure");
    if let Some(mut stdin) = child.stdin.take() { stdin.write_all(text.as_bytes()).unwrap(); }
    let _ = child.wait();
}

fn copy_from_clipboard(cpyf: &str) -> String {
    let (cmd, args) = split_cmd_args(cpyf);
    let out = Command::new(cmd)
        .args(args.split_whitespace())
        .output().expect("command CPY_FROM failure");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

// -------------------- run --------------------

pub fn run(jisyo: Jisyo, cpyt: &str, cpyf: &str) -> io::Result<()> {
    let tty_in = OpenOptions::new().read(true).open("/dev/tty")?;
    let mut ui = OpenOptions::new().read(true).write(true).open("/dev/tty")?
        .into_raw_mode()?
        .into_alternate_screen()?;

    let (mut buffer, mut prev) = (Buffer::default(), Buffer::default());
    let mut state = InputState::new_kana();
    let mut vp = Viewport::default();

    let _ = write!(ui, "{}", cursor::Show);
    let _ = ui.flush();

    redraw(&mut ui, &buffer, &state, &mut vp);

    for key in tty_in.keys() {
        let k = match key {
            Ok(k) => k,
            Err(_) => continue,
        };

        if let Some(cmd) = to_front_cmd(&k) {
            match cmd {
                FrontCmd::Quit => break,
                FrontCmd::Clear => {
                    prev = buffer.clone();
                    buffer.clear();
                    redraw(&mut ui, &buffer, &state, &mut vp);
                    continue;
                },
                FrontCmd::SendAndClear => {
                    prev = buffer.clone();
                    copy_to_clipboard(&buffer.as_string(), cpyt);
                    buffer.clear();
                    redraw(&mut ui, &buffer, &state, &mut vp);
                    continue;
                },
                FrontCmd::Paste => {
                    prev = buffer.clone();
                    buffer.insert_str(&copy_from_clipboard(cpyf));
                    redraw(&mut ui, &buffer, &state, &mut vp);
                    continue;
                },
                FrontCmd::Prev => {
                    (buffer, prev) = (prev,buffer);
                    redraw(&mut ui, &buffer, &state, &mut vp);
                    continue;
                }
            }
        }

        if let Some(ev) = to_key_event_with_state(&state, &k) {
            state = handle_key(state, &mut buffer, &jisyo, ev);
            redraw(&mut ui, &buffer, &state, &mut vp);
        }

    }

    let _ = write!(ui, "{}{}{}", clear::All, cursor::Goto(1, 1), cursor::Show);
    let _ = ui.flush();
    Ok(())
}

