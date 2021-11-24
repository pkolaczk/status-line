status-line
===========
**This crate allows you to display status & progress information in a terminal**

[![Crates.io](https://img.shields.io/crates/v/status-line.svg)](https://crates.io/crates/status-line)
[![Documentation](https://docs.rs/status-line/badge.svg)](https://docs.rs/status-line)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

This crate handles the problem of displaying a small amount of textual information in 
a terminal, periodically refreshing it, and finally erasing it, similar to how progress bars are displayed.

A status line can be viewed as a generalization of a progress bar.
Unlike progress bar drawing crates, this crate does not require
that you render the status text as a progress bar. It does not enforce any particular 
data format or template, nor it doesn't help you with formatting. 

The status line text may contain any information you wish, and may even be split
into multiple lines. You fully control the data model, as well as how the data gets printed
on the screen. The standard `Display` trait is used to convert the data into printed text.

Status updates can be made with a very high frequency, up to tens of millions of updates
per second. `StatusLine` decouples redrawing rate from the data update rate by using a
background thread to handle text printing with low frequency. 

## Example
```rust
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicU64, Ordering};
use status_line::StatusLine;

// Define the data model representing the status of your app.
// Make sure it is Send + Sync, so it can be read and written from different threads:
struct Progress(AtomicU64);

// Define how you want to display it:
impl Display for Progress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}%", self.0.load(Ordering::Relaxed))
    }
}

// StatusLine takes care of displaying the progress data:
let status = StatusLine::new(Progress(AtomicU64::new(0)));   // shows 0%
status.0.fetch_add(1, Ordering::Relaxed);                    // shows 1%
status.0.fetch_add(1, Ordering::Relaxed);                    // shows 2%
drop(status)                                                 // hides the status line
```
