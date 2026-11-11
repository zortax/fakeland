//! The run loop: the target, output changes, variants, and iterations with settle, trace and
//! frame captures.

use fakeland::{FrameCaptureId, Session, SessionError, WindowInfo};
use fakeland_scenario::WindowRef;
use fakeland_types::{Nanos, OutputId, SplitMix64};

use super::assemble::{Finished, assemble};
use super::capture::resolve;
use super::outputs::{apply, period_of, restore_changes};
use super::sleep::sleep;
use super::{Bench, BenchRun, DriverOutput, IterationContext, IterationRecord, Variant};
use crate::error::BenchError;
use crate::input::{AnalysisInput, IterationMeta, Target};
use crate::validity::{HostLoad, HostSample, lateness_of};

/// Salt of iteration seeds.
const SALT_ITERATION: u64 = 0x6974_6572;

/// Runs a bench.
pub async fn run(bench: &Bench, session: &Session) -> Result<BenchRun, BenchError> {
    let driver = bench.driver.clone().ok_or(BenchError::NoDriver)?;
    if bench.iterations == 0 {
        return Err(BenchError::NoIterations);
    }
    let clock = session.clock();
    let started = clock.now();
    let outputs_before = session.outputs().await?;
    let threshold = bench
        .validity
        .lateness_threshold
        .map(Nanos::from)
        .unwrap_or(session.config().lateness_threshold);
    let target = match &bench.target {
        Some(r) => Some(
            session
                .wait_window(r.clone(), bench.window_timeout.into())
                .await?,
        ),
        None => None,
    };
    let host_before = HostSample::read(&app_pids(session).await);
    let fallback = target.as_ref().map(|w| w.output);
    let result = iterations(bench, session, driver.as_ref(), target, fallback, threshold).await;
    let now = session.outputs().await.unwrap_or_default();
    let restore = restore_changes(&outputs_before, &now);
    let restored = apply(session, &restore, None).await;
    let records = result?;
    restored?;
    let host_after = HostSample::read(&app_pids(session).await);
    let host = host_before
        .zip(host_after)
        .and_then(|(a, b)| HostLoad::between(&a, &b));
    let finished = clock.now();
    Ok(assemble(
        bench,
        session,
        Finished {
            outputs: outputs_before,
            records,
            host,
            threshold,
            started,
            finished,
        },
    ))
}

async fn app_pids(session: &Session) -> Vec<u32> {
    let mut pids: Vec<u32> = session
        .clients()
        .await
        .unwrap_or_default()
        .iter()
        .filter_map(|c| c.pid)
        .map(|p| p.0)
        .collect();
    pids.sort_unstable();
    pids.dedup();
    pids
}

async fn iterations(
    bench: &Bench,
    session: &Session,
    driver: &dyn super::Driver,
    target: Option<WindowInfo>,
    fallback: Option<OutputId>,
    threshold: Nanos,
) -> Result<Vec<IterationRecord>, BenchError> {
    let variants: Vec<Option<Variant>> = if bench.variants.is_empty() {
        vec![None]
    } else {
        bench.variants.iter().cloned().map(Some).collect()
    };
    apply(session, &bench.outputs, fallback).await?;
    let mut records = Vec::new();
    let mut index = 0u32;
    for variant in variants {
        if let Some(v) = &variant {
            apply(session, &v.outputs, fallback).await?;
        }
        for i in 0..bench.warmup + bench.iterations {
            let measured = i.checked_sub(bench.warmup);
            let record = iteration(
                bench,
                session,
                driver,
                target.as_ref(),
                variant.clone(),
                index,
                measured,
                threshold,
            )
            .await?;
            index += 1;
            let failed = !record.ok();
            records.push(record);
            if failed && bench.stop_on_error {
                return Ok(records);
            }
        }
    }
    Ok(records)
}

#[allow(clippy::too_many_arguments)]
async fn iteration(
    bench: &Bench,
    session: &Session,
    driver: &dyn super::Driver,
    target: Option<&WindowInfo>,
    variant: Option<Variant>,
    index: u32,
    measured: Option<u32>,
    threshold: Nanos,
) -> Result<IterationRecord, BenchError> {
    let window = match target {
        Some(t) => Some(current_window(session, t).await?),
        None => None,
    };
    let outputs = session.outputs().await?;
    let output = window
        .as_ref()
        .and_then(|w| outputs.iter().find(|o| o.id == w.output))
        .or_else(|| outputs.first());
    let period = output.and_then(period_of);
    let seed = SplitMix64::hash(bench.seed, SALT_ITERATION, u64::from(index));
    let phase = bench
        .phase
        .zip(period)
        .and_then(|(p, period)| p.phase_for(measured.unwrap_or(index), period));
    let cx = IterationContext {
        index,
        measured,
        variant: variant.clone(),
        target: window.clone(),
        phase,
        period,
        seed,
    };
    sleep(session, bench.settle).await?;
    if let (Some(quiet), Some(w)) = (bench.quiet, &window) {
        match session
            .wait_settled(
                WindowRef::id(w.id),
                quiet.into(),
                bench.quiet_timeout.into(),
            )
            .await
        {
            Ok(_) | Err(SessionError::Timeout) => {}
            Err(e) => return Err(e.into()),
        }
    }
    if measured.is_some() {
        session.reset_validity().await?;
    }
    let started = session.clock().now();
    let capture = session.begin_capture().await?;
    let mut frame_ids: Vec<FrameCaptureId> = Vec::new();
    for spec in &bench.frames {
        frame_ids.push(
            session
                .begin_frame_capture(resolve(spec, window.as_ref()))
                .await?,
        );
    }
    let driven = driver.drive(session, &cx).await;
    let mut frames = Vec::with_capacity(frame_ids.len());
    for id in frame_ids {
        frames.push(session.end_frame_capture(id).await?);
    }
    let trace = session.end_capture(capture).await?;
    let finished = session.clock().now();
    let (out, error) = match driven {
        Ok(out) => {
            let e = out.error.clone();
            (out, e)
        }
        Err(BenchError::Session(SessionError::Closed)) => {
            return Err(BenchError::Session(SessionError::Closed));
        }
        Err(e) => (DriverOutput::default(), Some(e.to_string())),
    };
    let meta = IterationMeta {
        index,
        measured,
        variant: variant.map(|v| v.label),
        phase,
        seed,
        started,
        finished,
    };
    let lateness = lateness_of(&trace, threshold);
    let keep = measured.is_some() || bench.keep_warmups;
    let target = out.target.as_ref().or(window.as_ref()).map(Target::from);
    let input = keep.then(|| {
        frames.extend(out.frames);
        AnalysisInput::new(trace)
            .with_meta(meta.clone())
            .with_target(target)
            .with_probes(out.probes)
            .with_frames(frames)
            .with_run(out.run)
            .with_observations(out.observations)
            .with_values(out.values)
    });
    Ok(IterationRecord {
        meta,
        error,
        lateness,
        input,
    })
}

async fn current_window(session: &Session, target: &WindowInfo) -> Result<WindowInfo, BenchError> {
    session
        .windows()
        .await?
        .into_iter()
        .find(|w| w.id == target.id)
        .ok_or(BenchError::Session(SessionError::NoWindow))
}
