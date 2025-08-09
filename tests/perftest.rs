
use std::{fs::File, time::SystemTime};

use xdg_desktop::desktop_parser::{DesktopFile, DesktopParserCallback};


struct DummyCallback {}

impl DesktopParserCallback for DummyCallback {
    fn on_section(&mut self, _name: &[u8]) -> bool {
        true
    }

    fn on_key(&mut self, _key: &[u8]) -> bool {
        true
    }

    fn on_value(&mut self, _value: &[u8]) -> bool {
        true
    }
}

#[test]
fn test_parsing_performance() {
    let p = DesktopFile::new(File::open("./tests/sample.desktop").unwrap()).unwrap();
    let mut callback = DummyCallback {};
    let start = SystemTime::now();
    for _ in 0..1000000 {
        p.parse(&mut callback);
    }
    let end = SystemTime::now();
    println!("each iteration {}us", end.duration_since(start).unwrap().as_millis() as f32 / 1000.);
}
