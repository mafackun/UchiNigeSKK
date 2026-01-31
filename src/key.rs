#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    // --- 共通（編集・入力） ---
    Char(char),
    Backspace,
    Delete,

    // --- カーソル移動 ---
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    ToLineHead,
    ToLineTail,

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
