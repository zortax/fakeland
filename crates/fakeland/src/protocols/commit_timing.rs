//! `wp_commit_timing_manager_v1` in unmanaged mode. The commit hook takes the target time and the
//! display scheduler holds the update until its refresh.

use smithay::delegate_commit_timing;

use crate::server::State;

delegate_commit_timing!(State);
