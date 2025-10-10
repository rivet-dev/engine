use gas::prelude::*;
use rivet_types::runner_configs::{RunnerConfig, RunnerConfigKind};
use universaldb::{options::MutationType, utils::IsolationLevel::*};

use crate::{errors, keys, utils::runner_config_variant};

#[derive(Debug)]
pub struct Input {
	pub namespace_id: Id,
	pub name: String,
	pub config: RunnerConfig,
}

#[operation]
pub async fn namespace_runner_config_upsert(ctx: &OperationCtx, input: &Input) -> Result<()> {
	ctx.udb()?
		.run(|tx| async move {
			let tx = tx.with_subspace(keys::subspace());

			let runner_config_key =
				keys::runner_config::DataKey::new(input.namespace_id, input.name.clone());

			// Delete previous index
			if let Some(existing_config) = tx.read_opt(&runner_config_key, Serializable).await? {
				tx.delete(&keys::runner_config::ByVariantKey::new(
					input.namespace_id,
					runner_config_variant(&existing_config),
					input.name.clone(),
				));
			}

			// Write new config
			tx.write(&runner_config_key, input.config.clone())?;
			tx.write(
				&keys::runner_config::ByVariantKey::new(
					input.namespace_id,
					runner_config_variant(&input.config),
					input.name.clone(),
				),
				input.config.clone(),
			)?;

			match &input.config.kind {
				RunnerConfigKind::Normal { .. } => {}
				RunnerConfigKind::Serverless {
					url,
					headers,
					slots_per_runner,
					..
				} => {
					// Validate url
					if let Err(err) = url::Url::parse(url) {
						return Ok(Err(errors::RunnerConfig::Invalid {
							reason: format!("invalid serverless url: {err}"),
						}));
					}

					if headers.len() > 16 {
						return Ok(Err(errors::RunnerConfig::Invalid {
							reason: "too many headers (max 16)".to_string(),
						}));
					}

					for (n, v) in headers {
						if n.len() > 128 {
							return Ok(Err(errors::RunnerConfig::Invalid {
								reason: format!("invalid header name: too long (max 128)"),
							}));
						}
						if let Err(err) = n.parse::<reqwest::header::HeaderName>() {
							return Ok(Err(errors::RunnerConfig::Invalid {
								reason: format!("invalid header name: {err}"),
							}));
						}
						if v.len() > 4096 {
							return Ok(Err(errors::RunnerConfig::Invalid {
								reason: format!("invalid header value: too long (max 4096)"),
							}));
						}
						if let Err(err) = v.parse::<reqwest::header::HeaderValue>() {
							return Ok(Err(errors::RunnerConfig::Invalid {
								reason: format!("invalid header value: {err}"),
							}));
						}
					}

					// Validate slots per runner
					if *slots_per_runner == 0 {
						return Ok(Err(errors::RunnerConfig::Invalid {
							reason: "`slots_per_runner` cannot be 0".to_string(),
						}));
					}

					// Sets desired count to 0 if it doesn't exist
					let tx = tx.with_subspace(rivet_types::keys::pegboard::subspace());
					tx.atomic_op(
						&rivet_types::keys::pegboard::ns::ServerlessDesiredSlotsKey::new(
							input.namespace_id,
							input.name.clone(),
						),
						&0i64.to_le_bytes(),
						MutationType::Add,
					);
				}
			}

			Ok(Ok(()))
		})
		.custom_instrument(tracing::info_span!("runner_config_upsert_tx"))
		.await?
		.map_err(|err| err.build())?;

	// Bump autoscaler
	if input.config.affects_autoscaler() {
		ctx.msg(rivet_types::msgs::pegboard::BumpServerlessAutoscaler {})
			.send()
			.await?;
	}

	Ok(())
}
