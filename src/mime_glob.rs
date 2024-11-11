use core::str;
use std::{collections::HashMap, fs::File};
use std::io::Result;

use glob::Pattern;
use memmap::MmapOptions;

struct MIMEGlobItem {
    score: usize,
    mime: String,
    pattern: Option<Pattern>,
}

fn parse_mime_glob<'a, Callback>(slice: &'a [u8], mut callback: Callback) where Callback: FnMut(&'a [u8], &'a [u8], &'a [u8]) -> bool {
    let mut line_start = 0;
    while line_start < slice.len() {
        let Some(line_size) = slice[line_start..].iter().position(|ch| *ch == b'\n') else {
            break;
        };

        if slice[line_start] != b'#' {
            let line_args = slice[line_start..line_start + line_size].split(|ch| *ch == b':').into_iter().take(3).collect::<Vec<&'a [u8]>>();
            if line_args.len() < 3 {
                line_start += line_size + 1;
                continue;
            }
            if !callback(line_args[0], line_args[1], line_args[2]) {
                break;
            }
        }

        line_start += line_size + 1;
    }
}

pub fn mime_glob_foreach<ForCallback>(
    mut for_callback: ForCallback) -> Result<()>
where ForCallback: FnMut(usize, String, &str) -> bool {
    let file = File::open("/usr/share/mime/globs2")?;
    let region = unsafe { MmapOptions::new().map(&file)? };
    parse_mime_glob(region.iter().as_slice(), |score, mime, ptn| {
        let Ok(Ok(score)) = str::from_utf8(score).map(|s| s.parse::<usize>()) else {
            return true; // Skip.
        };

        for_callback(score,
                     String::from_utf8(mime.to_vec()).unwrap(),
                     str::from_utf8(ptn).unwrap())
    });

    Ok(())
}

pub struct MIMEGlobIndex {
    glob_patterns: Vec<MIMEGlobItem>,
    glob_suffix_index: HashMap<String, MIMEGlobItem>,
}

impl MIMEGlobIndex {
    pub fn new() -> Result<Self> {
        let mut glob_patterns: Vec<MIMEGlobItem> = vec![];
        let mut glob_suffix_index: HashMap<String, MIMEGlobItem> = HashMap::new();

        mime_glob_foreach(|score, mime, ptn| {
            if ptn.chars().nth(0) == Some('*') && ptn[1..].chars().all(|ch| ch != '*' && ch != '?') {
                glob_suffix_index.insert(ptn[1..].to_string(), MIMEGlobItem {
                    score, mime, pattern: None,
                });
            } else {
                glob_patterns.push(MIMEGlobItem {
                    score,
                    mime,
                    pattern: Some(Pattern::new(ptn).unwrap()),
                });
            }

            true
        })?;

        Ok(Self {
            glob_patterns, glob_suffix_index,
        })
    }

    fn match_filename_suffix(&self, filename: &str) -> Option<&MIMEGlobItem> {
        if let Some(extpos) = filename.rfind('.') {
            return self.glob_suffix_index.get(&filename[extpos..]);
        }

        None
    }

    fn match_filename_pattern(&self, filename: &str, min_score: usize) -> Option<&MIMEGlobItem> {
        for glob_item in &self.glob_patterns {
            if glob_item.score < min_score {
                return None;
            }
            if glob_item.pattern.as_ref().unwrap().matches(filename) {
                return Some(glob_item);
            }
        }

        None
    }

    pub fn match_filename(&self, filename: &str) -> Option<&str> {
        let suffix_match = self.match_filename_suffix(filename);
        let suffix_score = suffix_match.map(|item| item.score).unwrap_or(0);

        let pattern_match = self.match_filename_pattern(filename, suffix_score);
        let pattern_score = pattern_match.map(|item| item.score).unwrap_or(0);
        if suffix_score > pattern_score {
            suffix_match.map(|item| item.mime.as_str())
        } else {
            pattern_match.map(|item| item.mime.as_str())
        }
    }

}
