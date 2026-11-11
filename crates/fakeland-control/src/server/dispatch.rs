//! Maps a method name to its operation: parse the parameters, run the operation, encode the
//! result.

use std::future::Future;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::methods::*;
use crate::ops::{Controller, OpError};
use crate::wire::{ErrorBody, ErrorCode};

/// Parses parameters. `null` reads as an empty object.
pub fn parse<P: DeserializeOwned>(params: Value) -> Result<P, ErrorBody> {
    let params = if params.is_null() {
        Value::Object(serde_json::Map::new())
    } else {
        params
    };
    serde_json::from_value(params)
        .map_err(|e| ErrorBody::new(ErrorCode::InvalidParams, e.to_string()))
}

/// Encodes a result.
pub fn encode<T: serde::Serialize>(value: T) -> Result<Value, ErrorBody> {
    serde_json::to_value(value).map_err(|e| ErrorBody::new(ErrorCode::Failed, e.to_string()))
}

async fn run<M, Fut>(params: Value, f: impl FnOnce(M::Params) -> Fut) -> Result<Value, ErrorBody>
where
    M: Method,
    Fut: Future<Output = Result<M::Output, OpError>>,
{
    let p = parse::<M::Params>(params)?;
    encode(f(p).await.map_err(|e| e.to_body())?)
}

/// Runs one method that needs no connection state.
pub async fn dispatch(c: &Controller, method: &str, params: Value) -> Result<Value, ErrorBody> {
    match method {
        Info::NAME => run::<Info, _>(params, |p| async move { Ok(c.info(p)) }).await,
        A11yApps::NAME => run::<A11yApps, _>(params, |p| c.a11y_apps(p)).await,
        A11yTreeOf::NAME => run::<A11yTreeOf, _>(params, |p| c.a11y_tree(p)).await,
        A11yFind::NAME => run::<A11yFind, _>(params, |p| c.a11y_find(p)).await,
        A11yNode::NAME => run::<A11yNode, _>(params, |p| c.a11y_node(p)).await,
        A11yAction::NAME => run::<A11yAction, _>(params, |p| c.a11y_action(p)).await,
        A11ySetText::NAME => run::<A11ySetText, _>(params, |p| c.a11y_set_text(p)).await,
        A11yFocus::NAME => run::<A11yFocus, _>(params, |p| c.a11y_focus(p)).await,
        Validity::NAME => run::<Validity, _>(params, |p| c.validity(p)).await,
        ResetValidity::NAME => run::<ResetValidity, _>(params, |p| c.reset_validity(p)).await,
        Mark::NAME => run::<Mark, _>(params, |p| c.mark(p)).await,
        Step::NAME => run::<Step, _>(params, |p| c.step(p)).await,
        Windows::NAME => run::<Windows, _>(params, |p| c.windows(p)).await,
        Resources::NAME => run::<Resources, _>(params, |p| c.resources(p)).await,
        RecordStart::NAME => run::<RecordStart, _>(params, |p| c.record_start(p)).await,
        RecordStop::NAME => run::<RecordStop, _>(params, |p| c.record_stop(p)).await,
        Recordings::NAME => run::<Recordings, _>(params, |p| c.recordings(p)).await,
        Clients::NAME => run::<Clients, _>(params, |p| c.clients(p)).await,
        WaitWindow::NAME => run::<WaitWindow, _>(params, |p| c.wait_window(p)).await,
        WaitSettled::NAME => run::<WaitSettled, _>(params, |p| c.wait_settled(p)).await,
        WaitFrames::NAME => run::<WaitFrames, _>(params, |p| c.wait_frames(p)).await,
        WaitFor::NAME => run::<WaitFor, _>(params, |p| c.wait(p)).await,
        WindowOperation::NAME => run::<WindowOperation, _>(params, |p| c.window_op(p)).await,
        Launch::NAME => run::<Launch, _>(params, |p| c.launch(p)).await,
        Apps::NAME => run::<Apps, _>(params, |p| async move { Ok(c.apps_list(p)) }).await,
        Kill::NAME => run::<Kill, _>(params, |p| c.kill(p)).await,
        Restart::NAME => run::<Restart, _>(params, |p| c.restart(p)).await,
        AppLogs::NAME => run::<AppLogs, _>(params, |p| async move { c.logs(p) }).await,
        Outputs::NAME => run::<Outputs, _>(params, |p| c.outputs(p)).await,
        SetOutput::NAME => run::<SetOutput, _>(params, |p| c.set_output(p)).await,
        AddOutput::NAME => run::<AddOutput, _>(params, |p| c.add_output(p)).await,
        RemoveOutput::NAME => run::<RemoveOutput, _>(params, |p| c.remove_output(p)).await,
        Screenshot::NAME => run::<Screenshot, _>(params, |p| c.screenshot(p)).await,
        Inject::NAME => run::<Inject, _>(params, |p| c.inject(p)).await,
        Act::NAME => run::<Act, _>(params, |p| c.act(p)).await,
        PointerMove::NAME => run::<PointerMove, _>(params, |p| c.pointer_move(p)).await,
        Click::NAME => run::<Click, _>(params, |p| c.click(p)).await,
        Scroll::NAME => run::<Scroll, _>(params, |p| c.scroll(p)).await,
        DragTo::NAME => run::<DragTo, _>(params, |p| c.drag(p)).await,
        KeyTap::NAME => run::<KeyTap, _>(params, |p| c.key(p)).await,
        KeyCombo::NAME => run::<KeyCombo, _>(params, |p| c.combo(p)).await,
        TypeText::NAME => run::<TypeText, _>(params, |p| c.type_text(p)).await,
        TouchTap::NAME => run::<TouchTap, _>(params, |p| c.touch_tap(p)).await,
        ClipboardGet::NAME => run::<ClipboardGet, _>(params, |p| c.clipboard_get(p)).await,
        ClipboardSet::NAME => run::<ClipboardSet, _>(params, |p| c.clipboard_set(p)).await,
        RunResultOf::NAME => run::<RunResultOf, _>(params, |p| c.run_result(p)).await,
        CancelRun::NAME => run::<CancelRun, _>(params, |p| c.cancel_run(p)).await,
        Runs::NAME => run::<Runs, _>(params, |p| async move { Ok(c.runs_list(p)) }).await,
        Probe::NAME => run::<Probe, _>(params, |p| c.probe(p)).await,
        InstallTriggers::NAME => run::<InstallTriggers, _>(params, |p| c.install_triggers(p)).await,
        RemoveTriggers::NAME => run::<RemoveTriggers, _>(params, |p| c.remove_triggers(p)).await,
        TraceBegin::NAME => run::<TraceBegin, _>(params, |p| c.trace_begin(p)).await,
        TraceEnd::NAME => run::<TraceEnd, _>(params, |p| c.trace_end(p)).await,
        TraceSnapshot::NAME => run::<TraceSnapshot, _>(params, |p| c.trace_snapshot(p)).await,
        FrameStatsOf::NAME => run::<FrameStatsOf, _>(params, |p| c.frame_stats(p)).await,
        ListPresets::NAME => {
            run::<ListPresets, _>(params, |p| async move { Ok(c.presets(p)) }).await
        }
        RunBench::NAME => run::<RunBench, _>(params, |p| c.bench(p)).await,
        other => Err(ErrorBody::new(
            ErrorCode::UnknownMethod,
            format!("unknown method {other:?}"),
        )),
    }
}
