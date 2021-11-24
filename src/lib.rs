//! # status-line
//!
//! This crate handles the problem of displaying a small amount of textual information in
//! a terminal, periodically refreshing it, and finally erasing it, similar to how progress bars
//! are displayed.
//!
//! A status line can be viewed as a generalization of a progress bar.
//! Unlike progress bar drawing crates, this crate does not require
//! that you render the status text as a progress bar. It does not enforce any particular
//! data format or template, nor it doesn't help you with formatting.
//!
//! The status line text may contain any information you wish, and may even be split
//! into multiple lines. You fully control the data model, as well as how the data gets printed
//! on the screen. The standard `Display` trait is used to convert the data into printed text.
//!
//! Status updates can be made with a very high frequency, up to tens of millions of updates
//! per second. `StatusLine` decouples redrawing rate from the data update rate by using a
//! background thread to handle text printing with low frequency.
//!
//! ## Example
//! ```rust
//! use std::fmt::{Display, Formatter};
//! use std::sync::atomic::{AtomicU64, Ordering};
//! use status_line::StatusLine;
//!
//! // Define the data model representing the status of your app.
//! // Make sure it is Send + Sync, so it can be read and written from different threads:
//! struct Progress(AtomicU64);
//!
//! // Define how you want to display it:
//! impl Display for Progress {
//!     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//!         write!(f, "{}%", self.0.load(Ordering::Relaxed))
//!     }
//! }
//!
//! // StatusLine takes care of displaying the progress data:
//! let status = StatusLine::new(Progress(AtomicU64::new(0)));   // shows 0%
//! status.0.fetch_add(1, Ordering::Relaxed);                    // shows 1%
//! status.0.fetch_add(1, Ordering::Relaxed);                    // shows 2%
//! drop(status)                                                 // hides the status line
//! ```
//!

use std::fmt::Display;
use std::io::Write;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use ansi_escapes::{CursorLeft, CursorPrevLine, EraseDown};

fn redraw(state: &impl Display) {
    let stderr = std::io::stderr();
    let mut stderr = stderr.lock();
    let contents = format!("{}", state);
    let line_count = contents.chars().filter(|c| *c == '\n').count();
    write!(&mut stderr, "{}{}{}", EraseDown, state, CursorLeft).unwrap();
    for _ in 0..line_count {
        write!(&mut stderr, "{}", CursorPrevLine).unwrap();
    }
}

fn clear() {
    let stderr = std::io::stderr();
    let mut stderr = stderr.lock();
    write!(&mut stderr, "{}", EraseDown).unwrap();
}

struct State<D> {
    data: D,
    visible: AtomicBool,
}

impl<D> State<D> {
    pub fn new(inner: D) -> State<D> {
        State {
            data: inner,
            visible: AtomicBool::new(false),
        }
    }
}

/// Options controlling how to display the status line
pub struct Options {
    /// How long to wait between subsequent refreshes of the status.
    /// Defaults to 100 ms.
    pub refresh_period: Duration,
    /// Set it to false if you don't want to show the status on creation of the `StatusLine`.
    /// You can change the visibility of the `StatusLine` any time by calling
    /// [`StatusLine::set_visible`].
    pub initially_visible: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            refresh_period: Duration::from_millis(100),
            initially_visible: true,
        }
    }
}

/// Wraps arbitrary data and displays it periodically on the screen.
pub struct StatusLine<D: Display> {
    state: Arc<State<D>>,
}

impl<D: Display + Send + Sync + 'static> StatusLine<D> {
    /// Creates a new `StatusLine` with default options and shows it immediately.
    pub fn new(data: D) -> StatusLine<D> {
        Self::with_options(data, Default::default())
    }

    /// Creates a new `StatusLine` with custom options.
    pub fn with_options(data: D, options: Options) -> StatusLine<D> {
        let state = Arc::new(State::new(data));
        state
            .visible
            .store(options.initially_visible, Ordering::Release);
        let state_ref = state.clone();
        thread::spawn(move || {
            while Arc::strong_count(&state_ref) > 1 {
                if state_ref.visible.load(Ordering::Acquire) {
                    redraw(&state_ref.data);
                }
                thread::sleep(options.refresh_period);
            }
        });
        StatusLine { state }
    }
}

impl<D: Display> StatusLine<D> {
    /// Forces redrawing the status information immediately,
    /// without waiting for the next refresh cycle of the background refresh loop.
    pub fn refresh(&self) {
        redraw(&self.state.data);
    }

    /// Sets the visibility of the status line.
    pub fn set_visible(&self, visible: bool) {
        let was_visible = self.state.visible.swap(visible, Ordering::Release);
        if !visible && was_visible {
            clear()
        } else if visible && !was_visible {
            redraw(&self.state.data)
        }
    }

    /// Returns true if the status line is currently visible.
    pub fn is_visible(&self) -> bool {
        self.state.visible.load(Ordering::Acquire)
    }
}

impl<D: Display> Deref for StatusLine<D> {
    type Target = D;
    fn deref(&self) -> &Self::Target {
        &self.state.data
    }
}

impl<D: Display> Drop for StatusLine<D> {
    fn drop(&mut self) {
        if self.is_visible() {
            clear()
        }
    }
}
