use status_line::StatusLine;
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

const PROGRESS_LEN: usize = 80;

struct Progress {
    pos: AtomicUsize,
    max: usize,
}

impl Display for Progress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let pos = self.pos.load(Ordering::Relaxed);
        let pos = PROGRESS_LEN * pos / self.max;
        write!(f, "[{}{}]", "*".repeat(pos), " ".repeat(PROGRESS_LEN - pos))
    }
}

fn main() {
    let progress = Progress {
        pos: AtomicUsize::new(0),
        max: 1000000000,
    };

    let progress_bar = StatusLine::new(progress);

    // StatusLine can be moved to another thread:
    thread::spawn(move || {
        for _ in 0..progress_bar.max {
            progress_bar.pos.fetch_add(1, Ordering::Relaxed);
        }
    })
    .join()
    .unwrap();
}
