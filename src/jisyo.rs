use std::io;

#[derive(Debug, Clone)]
struct SingleJisyo {
    text: String,
    line_starts: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct Jisyo(Vec<SingleJisyo>);

impl Jisyo {
    pub fn load(pathes: &str) -> io::Result<Self> {
        let mut ret = Vec::<SingleJisyo>::new();
        let it = pathes.split(':');
        for path in it { ret.push(SingleJisyo::load(path)?); }
        Ok(Jisyo(ret))
    }

    pub fn lookup(&self, yomi: &str) -> Option<Vec<String>> {
        let mut ret = Vec::<String>::new();
        let Jisyo(vec) = self;
        for j in vec { if let Some(mut c) = j.lookup(yomi) {ret.append(&mut c)} }
        if ret.is_empty() { None } else { Some(ret) }
    }
}

impl SingleJisyo {
    fn load(path: &str) -> io::Result<Self> {
        let text = std::fs::read_to_string(path)?;

        // 1) 有効行の行頭だけ収集
        let mut line_starts = Vec::new();
        let mut start = 0usize;

        // 先頭行
        if Self::is_valid_line(&text, start) {
            line_starts.push(start as u32);
        }

        // '\n' の次が次行の先頭
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                start = i + 1;
                if start < text.len() && Self::is_valid_line(&text, start) {
                    line_starts.push(start as u32);
                }
            }
        }

        // 2) yomi でソート（辞書がソート済みでも、安全側で一度やる）
        line_starts.sort_by(|&a, &b| {
            let ya = Self::yomi_at(&text, a as usize);
            let yb = Self::yomi_at(&text, b as usize);
            ya.cmp(yb)
        });

        Ok(Self { text, line_starts })
    }

    /// 見つからなければ None
    fn lookup(&self, yomi: &str) -> Option<Vec<String>> {
        let text = &self.text;

        let idx = self
            .line_starts
            .binary_search_by(|&start| Self::yomi_at(text, start as usize).cmp(yomi))
            .ok()?;

        let start = self.line_starts[idx] as usize;
        Some(Self::candidates_at(text, start))
    }

    // --------------------
    // internal helpers
    // --------------------

    fn is_valid_line(text: &str, start: usize) -> bool {
        // 行頭から改行までの slice を取って trim
        let line = Self::line_slice(text, start);
        let t = line.trim();
        !t.is_empty() && !t.starts_with(';')
    }

    fn line_slice<'a>(text: &'a str, start: usize) -> &'a str {
        let bytes = text.as_bytes();
        let mut end = start;
        while end < bytes.len() && bytes[end] != b'\n' {
            end += 1;
        }
        &text[start..end]
    }

    /// 行の yomi を返す（`yomi<space>/.../` の yomi 部分）
    /// 形式が崩れていて space が無い場合は行全体（trim前）を返す
    fn yomi_at<'a>(text: &'a str, start: usize) -> &'a str {
        let line = Self::line_slice(text, start);

        // 先頭の空白を許容するなら trim_start しても良いが、
        // SKK 辞書は基本「行頭から yomi」なので、ここは単純にする
        if let Some(sp) = line.find(' ') {
            &line[..sp]
        } else {
            line
        }
    }

    /// 行の候補一覧を返す（アノテーション剥がし無し）
    /// `yomi<space>/cand1/cand2/.../` を想定
    fn candidates_at(text: &str, start: usize) -> Vec<String> {
        let line = Self::line_slice(text, start);

        let Some((_yomi, rest)) = line.split_once(' ') else {
            return Vec::new();
        };

        if !rest.starts_with('/') {
            return Vec::new();
        }

        // 先頭と末尾の '/' を意識しつつ split
        rest.split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }
}

