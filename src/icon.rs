use std::{collections::HashMap, path::{Path, PathBuf}, ffi::OsString};
use regex::Regex;

#[derive(Clone)]
pub struct BitmapIconDescription {
    pub size: usize,
    pub scale: usize,
}

#[derive(Clone)]
pub enum IconDescription {
    Scalable,
    Bitmap(BitmapIconDescription),
}

pub struct Icon {
    pub name: String,
    pub path: PathBuf,
    pub desc: IconDescription,
}

pub struct IconIndex {
    pub index: HashMap<String, Vec<Icon>>,
}

impl Icon {
    pub fn pixel_size(&self) -> Option<usize> {
	match &self.desc {
	    IconDescription::Scalable => None,
	    IconDescription::Bitmap(desc) => Some(desc.size * desc.scale),
	}
    }
}

fn filename_is_image(filename: &OsString) -> bool {
    if let Some(s) = filename.to_str() {
	return s.ends_with(".png") || s.ends_with(".svg");
    }
    return false;
}

fn parse_desc(s: &str) -> Option<IconDescription> {
    if s == "scalable" {
	return Some(IconDescription::Scalable);
    }
    let re = Regex::new(r"(?<size>[0-9]+)x[0-9]+(?:@(?<scale>[0-9]+))?").unwrap();

    let Some(m) = re.captures(s) else {
	return None;
    };

    let size = usize::from_str_radix(&m[1], 10).unwrap();
    let scale = if m.name("scale").is_some() { usize::from_str_radix(&m["scale"], 10).unwrap() } else { 1 };
    // eprintln!("size {} scale {}", size, scale);

    return Some(IconDescription::Bitmap(BitmapIconDescription {
	size, scale,
    }));
}

impl IconIndex {
    fn scan_dir(&mut self, dir: &Path, icon_desc: &IconDescription) {
	let Ok(d) = dir.read_dir() else {
	    return;
	};
	for ent in d {
	    let Ok(ent) = ent else {
		continue;
	    };
	    let path = ent.path();
	    let Ok(md) = path.metadata() else {
		eprintln!("Icon: Cannot open {}", path.to_str().unwrap());
		continue;
	    };
	    if md.is_file() && filename_is_image(&ent.file_name()) {
		self.add_image(&path, icon_desc);
	    } else if md.is_dir() {
		self.scan_dir(&path, icon_desc);
	    }
	}
    }

    fn add_image(&mut self, file: &Path, icon_desc: &IconDescription) -> () {
	let (Some(filename), Some(ext)) = (file.file_name(), file.extension()) else {
	    return;
	};
	let Some(filename_str) = filename.to_str() else {
	    return;
	};

	let icon_name = &filename_str[..filename_str.len() - ext.len() - 1];

	let symbolic_suffix = "-symbolic.symbolic";
	if icon_name.ends_with(symbolic_suffix) {
	    // Ignore them. They are ugly
	    return;
	}

	// eprintln!("Found icon {}", &icon_name);

	let icon = Icon {
	    name: String::from(icon_name), path: file.to_path_buf().clone(), desc: icon_desc.clone(),
	};

	if let Some(icons) = self.index.get_mut(icon_name) {
	    icons.push(icon);
	} else {
	    self.index.insert(String::from(icon_name), vec![icon]);
	}
    }

    fn scan_all_dir(&mut self, root_dir: &Path) {
	let Ok(dir) = root_dir.read_dir() else {
	    // eprintln!("Icon: Cannot read_dir: {}", root_dir.to_str().unwrap());
	    return;
	};
	for ent in dir {
	    let Ok(ent) = ent else {
		continue;
	    };
	    if let Ok(file_type) = ent.file_type() {
		if !file_type.is_dir() {
		    continue;
		}
		if let Some(icon_desc) = parse_desc(ent.file_name().to_str().unwrap()) {
		    self.scan_dir(&ent.path(), &icon_desc);
		}
	    };
	}
    }

    pub fn scan_with_theme(&mut self, themes: Vec<&str>, paths: &Vec<&str>) {
	for th in themes {
	    for path in paths {
		let mut pbuf = PathBuf::from(path);
		pbuf.push("icons");
		pbuf.push(th);
		self.scan_all_dir(pbuf.as_path());
	    }
	}
    }

    pub fn new() -> Self {
	IconIndex {
	    index: HashMap::new(),
	}
    }
}
