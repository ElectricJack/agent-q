//! One fixture-only committed B1 acknowledgement loss. Requests still reach the
//! real authenticated worker router; no result receipt is manufactured here.
use anyhow::{Context, Result, ensure};
use aq_admission::{Digest, result_record, transport::ResultReport};
use axum::{
    body::{Body, Bytes, to_bytes},
    extract::Request,
    middleware::Next,
    response::Response,
};
use serde_json::{Value, json};
use std::{
    pin::Pin,
    sync::Mutex,
    task::{Context as TaskContext, Poll},
};

#[derive(Default)]
pub struct LostB1 {
    in_flight: tokio::sync::Mutex<()>,
    observed: Mutex<Vec<(Vec<u8>, Vec<u8>)>>,
}
struct LostBody(bool);
impl http_body::Body for LostBody {
    type Data = Bytes;
    type Error = std::io::Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        _: &mut TaskContext<'_>,
    ) -> Poll<Option<std::result::Result<http_body::Frame<Bytes>, Self::Error>>> {
        if std::mem::replace(&mut self.0, true) {
            Poll::Ready(None)
        } else {
            Poll::Ready(Some(Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "fixture committed B1 acknowledgement loss",
            ))))
        }
    }
}
impl LostB1 {
    pub async fn forward(
        &self,
        request: Request,
        next: Next,
        observed: &super::codex_terminal_output::Observed,
    ) -> Result<Response> {
        let _slot = self
            .in_flight
            .try_lock()
            .context("B1 fixture exchange already in flight")?;
        ensure!(
            request.uri().path() == "/v1/worker/profiled/results",
            "exact B1 fixture route"
        );
        let (parts, body) = request.into_parts();
        let bytes = to_bytes(body, result_record::MAX_BYTES).await?;
        let report: ResultReport = serde_json::from_slice(&bytes)?;
        result_record::validate_report(&report)?;
        {
            let state = self.observed.lock().unwrap();
            ensure!(
                state.len() < 2
                    && state
                        .first()
                        .is_none_or(|first| first.0.as_slice() == bytes.as_ref()),
                "B1 fixture permits only exact original report replay"
            );
        }
        // Preserve actual authentication, current owner checks and transaction.
        let response = observed
            .inspect(Request::from_parts(parts, Body::from(bytes.clone())), next)
            .await;
        if !response.status().is_success() {
            return Ok(response);
        }
        let (mut parts, body) = response.into_parts();
        let reply = to_bytes(body, result_record::MAX_BYTES).await?;
        result_record::decode_receipt(&reply, &report)?;
        let first = {
            let mut state = self.observed.lock().unwrap();
            ensure!(
                state.len() < 2
                    && state
                        .first()
                        .is_none_or(|first| first.0.as_slice() == bytes.as_ref()
                            && first.1.as_slice() == reply.as_ref()),
                "committed B1 replay changed original request or receipt"
            );
            let first = state.is_empty();
            state.push((bytes.to_vec(), reply.to_vec()));
            first
        };
        if first {
            // Fully consumed the real committed reply above. The client sees a
            // transport body failure, not a fabricated or substituted receipt.
            parts.headers.remove(axum::http::header::CONTENT_LENGTH);
            Ok(Response::from_parts(parts, Body::new(LostBody(false))))
        } else {
            Ok(Response::from_parts(parts, Body::from(reply)))
        }
    }
    pub fn verify(&self, attempt: &Value, marker: &Value) -> Result<Value> {
        let state = self.observed.lock().unwrap();
        ensure!(
            state.len() == 2 && state[0] == state[1],
            "exact two B1 exchanges required"
        );
        let report: ResultReport = serde_json::from_slice(&state[0].0)?;
        let receipt = result_record::decode_receipt(&state[0].1, &report)?;
        let kept = &attempt["result"];
        ensure!(
            serde_json::to_value(&report)? == kept["report"]["request"]
                && serde_json::to_value(&receipt)? == kept["receipt"]
                && marker["schema_version"] == 1
                && marker["phase"] == "fixture_b1_replay_recorded"
                && marker["attempt_id"] == serde_json::to_value(report.binding.attempt_id)?
                && marker["operation_id"] == serde_json::to_value(report.operation_id)?
                && marker["report_id"] == serde_json::to_value(report.report_id)?,
            "B1 replay marker/history do not match actual committed exchanges"
        );
        let digest = result_record::report_digest(&report)?;
        ensure!(
            marker["report_digest"] == serde_json::to_value(&digest)?
                && kept["report"]["digest"] == serde_json::to_value(&digest)?,
            "B1 replay original canonical bytes differ"
        );
        ensure!(
            kept["report"]["bytes"] == serde_json::to_value(&state[0].0)?,
            "B1 replay original wire bytes differ"
        );
        Ok(
            json!({"exchanges":2,"committed_ack_lost_once":true,"same_request_bytes":true,
            "same_receipt_bytes":true,"report_digest":digest,"wire_request_digest":Digest::of(&state[0].0),"receipt_digest":Digest::of(&state[0].1),
            "original_error_preserved":true,"scope":"in-process exact history replay only; no restart or clean-run claim"}),
        )
    }
}
