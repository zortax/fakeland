//! `wp_fifo_manager_v1` in unmanaged mode. The commit hook reads the barrier requests and the
//! display scheduler resolves them.

use smithay::delegate_fifo;

use crate::server::State;

delegate_fifo!(State);
