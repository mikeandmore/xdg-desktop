use std::{collections::BTreeMap, fs, path::{Path, PathBuf}};
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

impl IconDescription {
    pub fn icon_size(&self) -> usize {
        match self {
            IconDescription::Scalable => { usize::max_value() },
            IconDescription::Bitmap(desc) => { desc.size * desc.scale },
        }
    }
}

struct IconDir {
    path: PathBuf,
    desc: IconDescription,
}

struct IconTheme {
    scalable_dirs: Vec<IconDir>,
    bitmap_dirs: BTreeMap<usize, Vec<IconDir>>,
}

pub struct IconCollection {
    themes: Vec<IconTheme>,
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

impl IconTheme {
    fn new(dirs: fs::ReadDir) -> Self {
        let mut this = Self {
            scalable_dirs: Vec::new(),
            bitmap_dirs: BTreeMap::new(),
        };
	for ent in dirs {
	    let Ok(ent) = ent else {
		continue;
	    };
	    if let Ok(file_type) = ent.file_type() {
		if !file_type.is_dir() && !file_type.is_symlink() {
		    continue;
		}
		if let Some(icon_desc) = parse_desc(ent.file_name().to_str().unwrap()) {
		    this.scan_dir(&ent.path(), &icon_desc);
		}
	    };
	}
        this
    }

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
            if md.is_dir() || md.is_symlink() {
                let dir = IconDir {
                    path: path.to_path_buf(),
                    desc: icon_desc.clone(),
                };
                match &icon_desc {
                    IconDescription::Scalable => { self.scalable_dirs.push(dir); }
                    IconDescription::Bitmap(desc) => {
                        let key = desc.size * desc.scale;
                        if let Some(dirs) = self.bitmap_dirs.get_mut(&key) {
                            dirs.push(dir);
                        } else {
                            self.bitmap_dirs.insert(key, vec![dir]);
                        }
                    }
                }
	    }
	}
    }

    fn find_icon_pred<Pred>(&self, name: &str, real_size: usize, mut pred: Pred) -> Option<(&IconDescription, PathBuf)>
    where Pred: FnMut(&IconDescription) -> bool + Copy {
        for it in &self.scalable_dirs {
            let mut p = it.path.clone();
            p.push(name.to_owned() + ".svg");
            if (p.is_file() || p.is_symlink()) && pred(&it.desc) {
                return Some((&it.desc, p));
            }
        }
        for it in self.bitmap_dirs.range(real_size..) {
            for suffix in [".svg", ".png"] {
                for desc in it.1 {
                    let mut p = desc.path.clone();
                    p.push(name.to_owned() + suffix);
                    // println!("Trying {}", p.display());
                    if p.is_file() || p.is_symlink() && pred(&desc.desc) {
                        return Some((&desc.desc, p));
                    }
                }
            }
        }

        None
    }
}

impl IconCollection {
    fn scan_all_dir(&mut self, root_dir: &Path) {
	let Ok(dirs) = root_dir.read_dir() else {
	    // eprintln!("Icon: Cannot read_dir: {}", root_dir.to_str().unwrap());
	    return;
	};
        self.themes.push(IconTheme::new(dirs))
    }

    pub fn scan_with_theme<'a, PathIterator>(&mut self, themes: Vec<&str>, paths: PathIterator)
    where PathIterator: Iterator<Item = &'a Path> {
        let pathbufs: Vec<PathBuf> = paths.map(|p| PathBuf::from(p)).collect();
	for th in themes {
	    for pbuf in &pathbufs {
		let mut pbuf = pbuf.clone();
                pbuf.push("icons");
                pbuf.push(th);
		self.scan_all_dir(pbuf.as_path());
	    }
	}
    }

    pub fn find_icon_pred<Pred>(&self, name: &str, real_size: usize, pred: Pred) -> Option<(&IconDescription, PathBuf)>
    where Pred: FnMut(&IconDescription) -> bool + Copy {
        for theme in &self.themes {
            let query = theme.find_icon_pred(name, real_size, pred);
            if query.is_some() {
                return query;
            }
        }

        None
    }

    pub fn find_icon(&self, name: &str, real_size: usize) -> Option<(&IconDescription, PathBuf)> {
        self.find_icon_pred(name, real_size, |_| true)
    }

    pub fn new() -> Self {
	IconCollection {
            themes: Vec::new(),
        }
    }
}
