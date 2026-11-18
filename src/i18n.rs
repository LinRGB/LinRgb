use std::sync::atomic::{AtomicBool, Ordering};

static FRENCH: AtomicBool = AtomicBool::new(false);

pub fn init(language: &str) {
    FRENCH.store(language == "fr", Ordering::Relaxed);
}

pub fn tr<'a>(english: &'a str, french: &'a str) -> &'a str {
    if FRENCH.load(Ordering::Relaxed) {
        french
    } else {
        english
    }
}
