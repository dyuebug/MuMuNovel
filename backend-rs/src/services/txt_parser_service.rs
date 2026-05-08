use encoding_rs::*;
use serde_json::{json, Value};

pub struct TxtParserService;

impl TxtParserService {
    pub fn decode_bytes(&self, content: &[u8]) -> (String, String) {
        // Try UTF-8 strict
        if let Ok(s) = String::from_utf8(content.to_vec()) {
            return (s, "utf-8".to_string());
        }

        // Try UTF-8 with BOM stripped
        if content.len() >= 3 && &content[..3] == b"\xef\xbb\xbf" {
            if let Ok(s) = String::from_utf8(content[3..].to_vec()) {
                return (s, "utf-8-sig".to_string());
            }
        }

        // Try GB18030 (covers GBK as subset)
        let (cow, _, had_errors) = GB18030.decode(content);
        if !had_errors || cow.len() > 0 {
            return (cow.into_owned(), "gb18030".to_string());
        }

        // Try Big5 (Traditional Chinese)
        let (cow, _, had_errors) = BIG5.decode(content);
        if !had_errors || cow.len() > 0 {
            return (cow.into_owned(), "big5".to_string());
        }

        // Fallback: UTF-8 with replacement
        let (cow, _, _) = UTF_8.decode(content);
        (cow.into_owned(), "utf-8(ignore)".to_string())
    }

    pub fn clean_text(&self, text: &str) -> String {
        let normalized = text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\u{feff}', "");

        let normalized = normalized.replace('\u{3000}', "  ");

        // Remove trailing spaces/tabs before newlines
        let mut result = String::with_capacity(normalized.len());
        for line in normalized.lines() {
            let trimmed = line.trim_end_matches(&[' ', '\t'] as &[_]);
            result.push_str(trimmed);
            result.push('\n');
        }

        // Compress 4+ consecutive newlines to 3
        let mut compressed = String::with_capacity(result.len());
        let mut newline_count = 0;
        for ch in result.chars() {
            if ch == '\n' {
                newline_count += 1;
                if newline_count <= 3 {
                    compressed.push(ch);
                }
            } else {
                newline_count = 0;
                compressed.push(ch);
            }
        }

        compressed.trim().to_string()
    }

    pub fn split_chapters(&self, text: &str) -> Vec<Value> {
        if text.trim().is_empty() {
            return vec![];
        }

        let lines: Vec<&str> = text.lines().collect();
        let mut heading_indexes: Vec<usize> = vec![];

        for (idx, line) in lines.iter().enumerate() {
            let stripped = line.trim();
            if stripped.is_empty() {
                continue;
            }
            if self.is_strong_heading(stripped) || self.is_weak_heading(&lines, idx) {
                heading_indexes.push(idx);
            }
        }

        heading_indexes.sort();
        heading_indexes.dedup();

        if heading_indexes.is_empty() {
            return self.fallback_split(text);
        }

        let mut chapters: Vec<Value> = vec![];
        let mut chapter_no = 1;

        let first_heading = heading_indexes[0];
        if first_heading > 0 {
            let preface: String = lines[..first_heading].join("\n");
            let preface = preface.trim().to_string();
            if preface.len() >= 200 {
                chapters.push(json!({
                    "title": "前言",
                    "content": preface,
                    "chapter_number": chapter_no,
                }));
                chapter_no += 1;
            }
        }

        for (i, &start_idx) in heading_indexes.iter().enumerate() {
            let end_idx = if i + 1 < heading_indexes.len() {
                heading_indexes[i + 1]
            } else {
                lines.len()
            };

            let title = {
                let t = lines[start_idx].trim();
                if t.len() > 200 {
                    &t[..200]
                } else {
                    t
                }
            };

            let body_start = start_idx + 1;
            let body = if body_start < end_idx {
                lines[body_start..end_idx].join("\n").trim().to_string()
            } else {
                String::new()
            };

            let body = if body.is_empty() && i + 1 < heading_indexes.len() {
                let next_line = if start_idx + 1 < lines.len() {
                    lines[start_idx + 1].trim().to_string()
                } else {
                    String::new()
                };
                next_line
            } else {
                body
            };

            let title_owned = if title.is_empty() {
                format!("第{}章", chapter_no)
            } else {
                title.to_string()
            };

            chapters.push(json!({
                "title": title_owned,
                "content": body,
                "chapter_number": chapter_no,
            }));
            chapter_no += 1;
        }

        let filtered: Vec<Value> = chapters
            .into_iter()
            .filter(|c| {
                let title = c["title"].as_str().unwrap_or("");
                let content = c["content"].as_str().unwrap_or("");
                !title.is_empty() || !content.is_empty()
            })
            .collect();

        if filtered.is_empty() {
            self.fallback_split(text)
        } else {
            filtered
        }
    }

    fn is_strong_heading(&self, line: &str) -> bool {
        self.match_chinese_chapter(line)
            || self.match_english_chapter(line)
            || self.match_chap_abbrev(line)
    }

    fn match_chinese_chapter(&self, line: &str) -> bool {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() || chars[0] != '第' {
            return false;
        }
        // Must have at least one Chinese/ASCII digit after 第
        let mut has_digit = false;
        let mut i = 1;
        while i < chars.len() {
            let ch = chars[i];
            if self.is_chinese_digit(ch)
                || ch.is_ascii_digit()
                || ch == '零'
                || ch == '〇'
                || ch == '两'
            {
                has_digit = true;
                i += 1;
            } else {
                break;
            }
        }
        if !has_digit || i >= chars.len() {
            return false;
        }
        // Next char should be a chapter unit
        let unit_chars = ['章', '节', '回', '卷', '集', '部', '篇'];
        unit_chars.contains(&chars[i])
    }

    fn is_chinese_digit(&self, ch: char) -> bool {
        matches!(
            ch,
            '一' | '二'
                | '三'
                | '四'
                | '五'
                | '六'
                | '七'
                | '八'
                | '九'
                | '十'
                | '百'
                | '千'
                | '万'
        )
    }

    fn match_english_chapter(&self, line: &str) -> bool {
        let lower = line.to_lowercase();
        let trimmed = lower.trim_start();
        if !trimmed.starts_with("chapter") {
            return false;
        }
        let after = &trimmed[7..]; // skip "chapter"
        let after = after.trim_start();
        after.chars().next().map_or(false, |c| c.is_ascii_digit())
    }

    fn match_chap_abbrev(&self, line: &str) -> bool {
        let lower = line.to_lowercase();
        let trimmed = lower.trim_start();
        if !trimmed.starts_with("chap.") {
            return false;
        }
        let after = &trimmed[5..]; // skip "chap."
        let after = after.trim_start();
        after.chars().next().map_or(false, |c| c.is_ascii_digit())
    }

    fn is_weak_heading(&self, lines: &[&str], idx: usize) -> bool {
        let line = lines[idx].trim();
        if line.is_empty() {
            return false;
        }
        if line.chars().count() > 25 {
            return false;
        }
        // Check for sentence-ending punctuation
        let punct = [
            '，', '。', '！', '？', '；', '：', ',', '.', '!', '?', ';', ':',
        ];
        if line.contains(&punct[..]) {
            return false;
        }
        let prev_blank = idx == 0 || lines[idx - 1].trim().is_empty();
        let next_blank = idx == lines.len() - 1 || lines[idx + 1].trim().is_empty();
        prev_blank && next_blank
    }

    fn fallback_split(&self, text: &str) -> Vec<Value> {
        let min_window = 3000usize;
        let max_window = 5000usize;
        let boundary: Vec<char> = "。！？!?\n".chars().collect();

        let mut chapters: Vec<Value> = vec![];
        let chars: Vec<char> = text.chars().collect();
        let n = chars.len();
        let mut start = 0;
        let mut chapter_no = 1;

        while start < n {
            let ideal_end = (start + max_window).min(n);
            let end = if ideal_end >= n {
                n
            } else {
                let search_from = (start + min_window).min(n);
                let segment: String = chars[search_from..ideal_end].iter().collect();
                let offset = boundary.iter().filter_map(|p| segment.rfind(*p)).max();
                match offset {
                    Some(o) => search_from + o + 1,
                    None => ideal_end,
                }
            };

            let chunk: String = chars[start..end].iter().collect();
            let chunk = chunk.trim().to_string();
            if !chunk.is_empty() {
                chapters.push(json!({
                    "title": format!("第{}章", chapter_no),
                    "content": chunk,
                    "chapter_number": chapter_no,
                }));
                chapter_no += 1;
            }
            start = end;
        }

        chapters
    }
}
