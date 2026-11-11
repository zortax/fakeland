//! `wp_presentation` on `CLOCK_MONOTONIC`. The commit hook takes the feedback objects and the
//! display scheduler sends `presented` or `discarded`.

use smithay::delegate_presentation;

use crate::server::State;

delegate_presentation!(State);
