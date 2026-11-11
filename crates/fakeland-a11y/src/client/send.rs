//! A compile-time check that the client futures are `Send`.

use super::*;
use crate::{NodeRef, Query, Scope, TreeOptions};

fn send<T: Send>(_: T) {}

/// Compiles only when the client futures are `Send`, so multi-threaded runtimes can spawn
/// them.
#[allow(dead_code)]
fn futures_are_send(a: &A11y, n: &NodeRef) {
    let scope = Scope::Desktop;
    let opts = TreeOptions::default();
    let q = Query::default();
    send(a.tree(&scope, &[], &opts));
    send(a.find(&scope, &q, &[], &opts, 1));
    send(a.apps(&[]));
    send(a.node(n, &[]));
    send(a.act(n, "click"));
    send(a.set_text(n, ""));
    send(a.focus(n));
    send(a.set_value(n, 0.0));
    send(A11y::connect(""));
}
