use core::str;
use std::{collections::HashMap, fs::File};
use std::io::Result;

use glob::Pattern;
use memmap::MmapOptions;

struct MIMEGlobItem {
    mime: String,
    pattern: Pattern,
}

fn parse_mime_glob<'a, Callback>(slice: &'a [u8], mut callback: Callback) where Callback: FnMut(&'a [u8], &'a [u8]) -> bool {
    let mut line_start = 0;
    loop {
        if line_start == slice.len() {
            break;
        }
        if slice[line_start] == b'#' {
            if let Some(line_end) = slice[line_start..].iter().position(|ch| *ch == b'\n') {
                line_start += line_end + 1;
                continue;
            } else {
                break;
            }
        }
        if let Some(colon_pos) = slice[line_start..].iter().position(|ch| *ch == b':') {
            if line_start + colon_pos + 1 == slice.len() {
                break;
            }
            if let Some(line_end) = slice[line_start + colon_pos + 1..].iter().position(|ch| *ch == b'\n') {
                let mime = &slice[line_start..line_start + colon_pos];
                let ptn = &slice[line_start + colon_pos + 1..line_start + colon_pos + 1 + line_end];
                if !callback(mime, ptn) {
                    return;
                }

                line_start += colon_pos + line_end + 2;
            } else {
                break;
            }
        } else {
            break;
        }
    }
}

pub fn mime_glob_foreach<ForCallback>(
    mut for_callback: ForCallback) -> Result<()>
where ForCallback: FnMut(String, &str) -> bool {
    let file = File::open("/usr/share/mime/globs")?;
    let region = unsafe { MmapOptions::new().map(&file)? };
    parse_mime_glob(region.iter().as_slice(), |mime, ptn| {
        for_callback(String::from_utf8(mime.to_vec()).unwrap(),
                     str::from_utf8(ptn).unwrap())
    });

    Ok(())
}

pub struct MIMEGlobIndex {
    glob_patterns: Vec<MIMEGlobItem>,
    glob_suffix_index: HashMap<String, String>,
}

impl MIMEGlobIndex {
    pub fn new() -> Result<Self> {
        let mut glob_patterns: Vec<MIMEGlobItem> = vec![];
        let mut glob_suffix_index: HashMap<String, String> = HashMap::new();

        mime_glob_foreach(|mime, ptn| {
            if ptn.chars().nth(0) == Some('*') && ptn[1..].chars().all(|ch| ch != '*') {
                glob_suffix_index.insert(ptn[1..].to_string(), mime);
            } else {
                glob_patterns.push(MIMEGlobItem {
                    mime,
                    pattern: Pattern::new(ptn).unwrap(),
                });
            }

            true
        })?;

        Ok(Self {
            glob_patterns, glob_suffix_index,
        })
    }

    fn match_filename_suffix(&self, filename: &str) -> Option<&str> {
        if let Some(extpos) = filename.rfind('.') {
            let extosstr = &filename[extpos..];
            if let Some(mime) = self.glob_suffix_index.get(extosstr) {
                return Some(mime);
            }
        }

        None
    }

    fn match_filename_regex(&self, filename: &str) -> Option<&str> {
        for glob_item in &self.glob_patterns {
            if glob_item.pattern.matches(filename) {
                return Some(glob_item.mime.as_str());
            }
        }

        None
    }

    pub fn match_filename(&self, filename: &str) -> Option<&str> {
        self.match_filename_suffix(filename).or_else(|| self.match_filename_regex(filename))
    }

}
