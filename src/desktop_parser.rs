use memmap::{MmapOptions, Mmap};
use std::fs::File;
use std::io::Result;

pub struct DesktopFile {
    pub file: File,
    mmap_region: Mmap,
}

pub trait DesktopParserCallback {
    fn on_section(&mut self, name: &[u8]);
    fn on_key(&mut self, key: &[u8]);
    fn on_value(&mut self, value: &[u8]);
}

fn skip_whitespace<'a>(slice: &'a[u8]) -> &'a [u8] {
    if let Some(pos) = slice.iter().position(|ch| { *ch != b' '}) {
	return &slice[pos..];
    } else {
	return &slice[..];
    }
}

fn find_next_char<'a>(x: u8, slice: &'a [u8]) -> Option<(&'a [u8], usize)> {
    let mut last:u8 = 0;
    let pos = slice.iter().position(|ch| {
	last = *ch;
	return *ch == b'\n' || *ch == x;
    });
    if pos.is_some() && last == x {
	return Some((&slice[pos.unwrap()..], pos.unwrap()));
    } else {
	return None;
    }
}

impl DesktopFile {
    pub fn new(file: File) -> Result<Self> {
	let mmap_region = unsafe { MmapOptions::new().map(&file)? };
	return Ok(Self {
	    file, mmap_region,
	});
    }
    pub fn parse(&self, callback: &mut impl DesktopParserCallback) {
	let mut slice = self.mmap_region.iter().as_slice();
	while slice.len() > 0 {
	    slice = skip_whitespace(slice);
	    if slice[0] == b'\n' {
		slice = &slice[1..];
	    } else if slice[0] == b'#' {
		slice = find_next_char(b'\n', slice).unwrap().0;
	    } else if slice[0] == b'[' {
		slice = &slice[1..];
		let (next_slice, pos) = find_next_char(b']', slice).unwrap();
		callback.on_section(&slice[..pos]);
		slice = &next_slice[1..]
	    } else {
		let (next_slice, pos) = find_next_char(b'=', slice).unwrap();
		callback.on_key(&slice[..pos]);
		slice = &next_slice[1..];
		let Some((next_slice, pos)) = find_next_char(b'\n', slice) else {
		    callback.on_value(&slice);
		    return;
		};
		callback.on_value(&slice[..pos]);
		slice = &next_slice[1..];
	    }
	}
    }
}

