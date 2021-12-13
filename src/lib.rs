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

fn redraw(ansi: bool, state: &impl Display) {
    let stderr = std::io::stderr();
    let mut stderr = stderr.lock();
    let contents = format!("{}", state);
    if ansi {
        let line_count = contents.chars().filter(|c| *c == '\n').count();
        write!(&mut stderr, "{}{}{}", EraseDown, contents, CursorLeft).unwrap();
        for _ in 0..line_count {
            write!(&mut stderr, "{}", CursorPrevLine).unwrap();
        }
    } else {
        writeln!(&mut stderr, "{}", contents).unwrap();
    }
}

fn clear(ansi: bool) {
    if ansi {
        let stderr = std::io::stderr();
        let mut stderr = stderr.lock();
        write!(&mut stderr, "{}", EraseDown).unwrap();
    }
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
    /// Defaults to 100 ms on interactive terminals (TTYs) and 1 s if the standard error
    /// is not interactive, e.g. redirected to a file.
    pub refresh_period: Duration,

    /// Set it to false if you don't want to show the status on creation of the `StatusLine`.
    /// You can change the visibility of the `StatusLine` any time by calling
    /// [`StatusLine::set_visible`].
    pub initially_visible: bool,

    /// Set to true to enable ANSI escape codes.
    /// By default set to true if the standard error is a TTY.
    /// If ANSI escape codes are disabled, the status line is not erased before each refresh,
    /// it is printed in a new line instead.
    pub enable_ansi_escapes: bool,
}

impl Default for Options {
    fn default() -> Self {
        let is_tty = atty::is(atty::Stream::Stderr);
        let refresh_period_ms = if is_tty { 100 } else { 1000 };
        Options {
            refresh_period: Duration::from_millis(refresh_period_ms),
            initially_visible: true,
            enable_ansi_escapes: is_tty,
        }
    }
}

/// Wraps arbitrary data and displays it periodically on the screen.
pub struct StatusLine<D: Display> {
    state: Arc<State<D>>,
    options: Options,
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
                    redraw(options.enable_ansi_escapes, &state_ref.data);
                }
                thread::sleep(options.refresh_period);
            }
        });
        StatusLine { state, options }
    }
}

impl<D: Display> StatusLine<D> {
    /// Forces redrawing the status information immediately,
    /// without waiting for the next refresh cycle of the background refresh loop.
    pub fn refresh(&self) {
        redraw(self.options.enable_ansi_escapes, &self.state.data);
    }

    /// Sets the visibility of the status line.
    pub fn set_visible(&self, visible: bool) {
        let was_visible = self.state.visible.swap(visible, Ordering::Release);
        if !visible && was_visible {
            clear(self.options.enable_ansi_escapes)
        } else if visible && !was_visible {
            redraw(self.options.enable_ansi_escapes, &self.state.data)
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
            clear(self.options.enable_ansi_escapes)
        }
    }
}
