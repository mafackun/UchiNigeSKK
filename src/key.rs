#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Left,
    Right,
    Up,
    Down,
    RapidUp,
    RapidDown,
    LineHead,
    LineTail,
    SelectLeft,
    SelectRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    // --- 共通（編集・入力） ---
    Char(char),
    Backspace,
    Delete,

    Navigation(Move),

    // --- モード切替 ---
    ToggleLatin,
    ToggleKatakana,
    ToggleHankakuZenkaku,

    // --- かな ---
    CommitUnconverted,
    Setsuji,
    StartYomiOrOkuri(char),

    // --- 変換 ---
    StartConversion,
    StartAbbrev,

    // --- 候補選択 ---
    NextCandidate,
    PrevCandidate,
    CommitCandidate,
    CommitCandidateWithChar(char),
    CommitCandidateWithStartYomi(char),
    CommitCandidateWithSetsubiji,
    CancelConversion,
}
