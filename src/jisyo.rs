use std::io;

struct SingleJisyo {
    text: Vec<u8>,
    line_starts: Vec<u32>,
}

pub struct Jisyo(Vec<SingleJisyo>);

impl Jisyo {
    pub fn load(pathes: &str) -> io::Result<Self> {
        let mut ret = Vec::<SingleJisyo>::new();
        let it = pathes.split(':');
        for path in it {
            ret.push(SingleJisyo::load(path)?);
        }
        Ok(Jisyo(ret))
    }

    pub fn lookup(&self, yomi: &str) -> Option<Vec<String>> {
        let mut ret = Vec::<String>::new();
        let Jisyo(vec) = self;
        for j in vec {
            if let Some(mut c) = j.lookup(yomi) {
                ret.append(&mut c)
            }
        }
        if ret.is_empty() { None } else { Some(ret) }
    }
}

impl SingleJisyo {
    fn load(path: &str) -> io::Result<Self> {
        let text = std::fs::read(path)?;
        let mut line_starts = Vec::new();

        if Self::is_valid_line(Self::line_slice(&text, 0)) {
            line_starts.push(0);
        }

        for (i, b) in text.iter().enumerate() {
            if *b == b'\n' && Self::is_valid_line(Self::line_slice(&text, i as u32 + 1)) {
                line_starts.push(i as u32 + 1);
            }
        }

        line_starts.sort_unstable_by(|&a, &b| {
            let ya = Self::yomi_at(&text[a as usize..]);
            let yb = Self::yomi_at(&text[b as usize..]);
            ya.cmp(yb)
        });

        Ok(Self { text, line_starts })
    }

    fn lookup(&self, yomi: &str) -> Option<Vec<String>> {
        let text = &self.text;
        let yomi = yomi.as_bytes();

        let idx = self
            .line_starts
            .binary_search_by(|&start| Self::yomi_at(&text[start as usize..]).cmp(yomi))
            .ok()?;

        Self::candidates_at(Self::line_slice(text, self.line_starts[idx]))
    }

    fn is_valid_line(line: &[u8]) -> bool {
        !line.is_empty() && line[0] != b';'
    }

    fn line_slice(text: &[u8], start: u32) -> &[u8] {
        let start = start as usize;
        let mut end = start;
        while end < text.len() && text[end] != b'\n' {
            end += 1;
        }
        &text[start..end]
    }

    fn yomi_at(line: &[u8]) -> &[u8] {
        for (i, b) in line.iter().enumerate() {
            match *b {
                b' ' => return &line[..i],
                b'\n' => break,
                _ => (),
            }
        }
        panic!("jisyo entry line does not contain whitespace")
    }

    fn candidates_at(line: &[u8]) -> Option<Vec<String>> {
        let line = str::from_utf8(line).expect("converting to utf8 failed");

        if let Some((_yomi, rest)) = line.split_once(' ')
            && rest.starts_with('/')
        {
            Some(
                rest.split('/')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect(),
            )
        } else {
            None
        }
    }
}
