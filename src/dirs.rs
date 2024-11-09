use std::{cmp::Ordering, env};

pub fn xdg_data_dirs() -> Vec<String> {
    let home_dir = env::var("HOME").unwrap_or("/root".to_string());
    let dirs = env::var("XDG_DATA_DIRS").unwrap_or_else(|_| {
        "/usr/share:/usr/local/share:".to_string() + home_dir.as_str() + "/.local/share"
    });
    let mut paths: Vec<&str> = dirs.split(':').collect();
    let rank_path = |s: &str| -> i32 {
        if s.starts_with("/usr") { -2 }
        else if s.starts_with("/usr/local") { -1 }
        else if s.starts_with(home_dir.as_str()) { 1 }
        else { 0 }
    };
    paths.sort_by(|a, b| {
        let ra = rank_path(a);
        let rb = rank_path(b);
        if ra < rb {
            return Ordering::Less;
        } else if ra > rb {
            return Ordering::Greater;
        } else {
            return a.cmp(b);
        }
    });

    let mut dedup_paths: Vec<String> = vec![];
    for p in paths {
        if dedup_paths.is_empty() || dedup_paths.last().unwrap().as_str().cmp(p) != Ordering::Equal {
            dedup_paths.push(p.to_string());
        }
    }

    dedup_paths
}
