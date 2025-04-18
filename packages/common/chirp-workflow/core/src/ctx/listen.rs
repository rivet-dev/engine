use std::time::Instant;

use crate::{
	ctx::WorkflowCtx,
	db::SignalData,
	error::{WorkflowError, WorkflowResult},
	history::location::Location,
	metrics,
};

/// Indirection struct to prevent invalid implementations of listen traits.
pub struct ListenCtx<'a> {
	ctx: &'a WorkflowCtx,
	location: &'a Location,
	// Used by certain db drivers to know when to update internal indexes for signal wake conditions
	last_try: bool,
	// HACK: Prevent `ListenCtx::listen_any` from being called more than once
	used: bool,
}

impl<'a> ListenCtx<'a> {
	pub(crate) fn new(ctx: &'a WorkflowCtx, location: &'a Location) -> Self {
		ListenCtx {
			ctx,
			location,
			last_try: false,
			used: false,
		}
	}

	pub(crate) fn reset(&mut self, last_try: bool) {
		self.used = false;
		self.last_try = last_try;
	}

	/// Checks for a signal to this workflow with any of the given signal names.
	/// - Will error if called more than once.
	#[tracing::instrument(skip_all, fields(?signal_names))]
	pub async fn listen_any(
		&mut self,
		signal_names: &[&'static str],
	) -> WorkflowResult<SignalData> {
		if self.used {
			return Err(WorkflowError::ListenCtxUsed);
		} else {
			self.used = true;
		}

		let start_instant = Instant::now();

		// Fetch new pending signal
		let signal = self
			.ctx
			.db()
			.pull_next_signal(
				self.ctx.workflow_id(),
				self.ctx.name(),
				signal_names,
				self.location,
				self.ctx.version(),
				self.ctx.loop_location(),
				self.last_try,
			)
			.await?;

		let dt = start_instant.elapsed().as_secs_f64();
		metrics::SIGNAL_PULL_DURATION
			.with_label_values(&[
				self.ctx.name(),
				signal
					.as_ref()
					.map(|signal| signal.signal_name.as_str())
					.unwrap_or("<none>"),
			])
			.observe(dt);

		let Some(signal) = signal else {
			return Err(WorkflowError::NoSignalFound(Box::from(signal_names)));
		};

		let recv_lag = (rivet_util::timestamp::now() as f64 - signal.create_ts as f64) / 1000.;
		crate::metrics::SIGNAL_RECV_LAG
			.with_label_values(&[self.ctx.name(), &signal.signal_name])
			.observe(recv_lag);

		tracing::debug!(
			signal_id=%signal.signal_id,
			signal_name=%signal.signal_name,
			"signal received",
		);

		Ok(signal)
	}
}
