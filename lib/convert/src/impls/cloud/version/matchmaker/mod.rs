use std::collections::{HashMap, HashSet};

use proto::{backend, common};
use rivet_api::models;
use rivet_operation::prelude::*;

use crate::{ApiFrom, ApiInto, ApiTryFrom, ApiTryInto};

pub mod game_mode;
pub mod lobby_group;

pub async fn config_to_proto(
	ctx: &OperationContext<()>,
	value: models::CloudVersionMatchmakerConfig,
) -> GlobalResult<backend::matchmaker::VersionConfig> {
	// Fetch region data required to convert models
	let mut all_region_name_ids = HashSet::new();
	if let Some(regions) = &value.regions {
		all_region_name_ids.extend(regions.keys().cloned());
	}
	if let Some(game_modes) = &value.game_modes {
		for game_mode in game_modes.values() {
			if let Some(regions) = &game_mode.regions {
				all_region_name_ids.extend(regions.keys().cloned());
			}
		}
	}

	let regions_list = op!([ctx] region_list {}).await?;
	let regions_res = op!([ctx] region_get {
		region_ids: regions_list.region_ids.clone(),
	})
	.await?;

	let lobby_groups = if let Some(x) = value.lobby_groups {
		x.into_iter()
			.map(ApiTryInto::api_try_into)
			.collect::<GlobalResult<Vec<_>>>()?
	} else if let Some(x) = value.game_modes.as_ref() {
		x.iter()
			.map(|(k, v)| game_mode::game_mode_to_proto(k.clone(), v, &value, &regions_res.regions))
			.collect::<GlobalResult<Vec<_>>>()?
	} else {
		Vec::new()
	};

	Ok(backend::matchmaker::VersionConfig {
		lobby_groups,
		captcha: value.captcha.map(|x| (*x).api_try_into()).transpose()?,
	})
}

pub async fn config_to_openapi(
	ctx: &OperationContext<()>,
	value: backend::matchmaker::VersionConfig,
) -> GlobalResult<models::CloudVersionMatchmakerConfig> {
	// Fetch region data required to convert models
	let all_region_ids = value
		.lobby_groups
		.iter()
		.flat_map(|x| x.regions.iter().flat_map(|x| x.region_id))
		.map(|x| x.as_uuid())
		.collect::<HashSet<_>>()
		.into_iter()
		.map(Into::<common::Uuid>::into)
		.collect::<Vec<_>>();
	let regions_res = op!([ctx] region_get {
		region_ids: all_region_ids,
	})
	.await?;

	Ok(models::CloudVersionMatchmakerConfig {
		game_modes: Some(
			value
				.lobby_groups
				.iter()
				.cloned()
				.map(|x| game_mode::game_mode_to_openapi(x, &regions_res.regions))
				.collect::<GlobalResult<HashMap<_, _>>>()?,
		),
		captcha: value
			.captcha
			.map(ApiTryInto::api_try_into)
			.transpose()?
			.map(Box::new),

		// Client-side configuration
		dev_hostname: None,

		// Overrides
		regions: None,
		max_players: None,
		max_players_direct: None,
		max_players_party: None,
		docker: None,
		tier: None,
		idle_lobbies: None,

		// Deprecated
		lobby_groups: Some(
			value
				.lobby_groups
				.iter()
				.cloned()
				.map(ApiTryFrom::api_try_from)
				.collect::<Result<Vec<_>, _>>()?,
		),
	})
}

impl ApiTryFrom<models::CloudVersionMatchmakerPortRange>
	for backend::matchmaker::lobby_runtime::PortRange
{
	type Error = GlobalError;

	fn api_try_from(value: models::CloudVersionMatchmakerPortRange) -> GlobalResult<Self> {
		ensure_with!(
			value.min >= 0,
			MATCHMAKER_INVALID_VERSION_CONFIG,
			error = "`port_range.min` out of bounds"
		);
		ensure_with!(
			value.max >= 0,
			MATCHMAKER_INVALID_VERSION_CONFIG,
			error = "`port_range.max` out of bounds"
		);

		Ok(backend::matchmaker::lobby_runtime::PortRange {
			min: value.min.api_try_into()?,
			max: value.max.api_try_into()?,
		})
	}
}

impl ApiTryFrom<backend::matchmaker::lobby_runtime::PortRange>
	for models::CloudVersionMatchmakerPortRange
{
	type Error = GlobalError;

	fn api_try_from(value: backend::matchmaker::lobby_runtime::PortRange) -> GlobalResult<Self> {
		Ok(models::CloudVersionMatchmakerPortRange {
			min: value.min.api_try_into()?,
			max: value.max.api_try_into()?,
		})
	}
}

impl ApiFrom<models::CloudVersionMatchmakerNetworkMode>
	for backend::matchmaker::lobby_runtime::NetworkMode
{
	fn api_from(
		value: models::CloudVersionMatchmakerNetworkMode,
	) -> backend::matchmaker::lobby_runtime::NetworkMode {
		match value {
			models::CloudVersionMatchmakerNetworkMode::Bridge => {
				backend::matchmaker::lobby_runtime::NetworkMode::Bridge
			}
			models::CloudVersionMatchmakerNetworkMode::Host => {
				backend::matchmaker::lobby_runtime::NetworkMode::Host
			}
		}
	}
}

impl ApiFrom<backend::matchmaker::lobby_runtime::NetworkMode>
	for models::CloudVersionMatchmakerNetworkMode
{
	fn api_from(value: backend::matchmaker::lobby_runtime::NetworkMode) -> Self {
		match value {
			backend::matchmaker::lobby_runtime::NetworkMode::Bridge => {
				models::CloudVersionMatchmakerNetworkMode::Bridge
			}
			backend::matchmaker::lobby_runtime::NetworkMode::Host => {
				models::CloudVersionMatchmakerNetworkMode::Host
			}
		}
	}
}

impl ApiFrom<models::CloudVersionMatchmakerPortProtocol>
	for backend::matchmaker::lobby_runtime::ProxyProtocol
{
	fn api_from(
		value: models::CloudVersionMatchmakerPortProtocol,
	) -> backend::matchmaker::lobby_runtime::ProxyProtocol {
		match value {
			models::CloudVersionMatchmakerPortProtocol::Http => {
				backend::matchmaker::lobby_runtime::ProxyProtocol::Http
			}
			models::CloudVersionMatchmakerPortProtocol::Https => {
				backend::matchmaker::lobby_runtime::ProxyProtocol::Https
			}
			models::CloudVersionMatchmakerPortProtocol::Tcp => {
				backend::matchmaker::lobby_runtime::ProxyProtocol::Tcp
			}
			models::CloudVersionMatchmakerPortProtocol::TcpTls => {
				backend::matchmaker::lobby_runtime::ProxyProtocol::TcpTls
			}
			models::CloudVersionMatchmakerPortProtocol::Udp => {
				backend::matchmaker::lobby_runtime::ProxyProtocol::Udp
			}
		}
	}
}

impl ApiFrom<backend::matchmaker::lobby_runtime::ProxyProtocol>
	for models::CloudVersionMatchmakerPortProtocol
{
	fn api_from(value: backend::matchmaker::lobby_runtime::ProxyProtocol) -> Self {
		match value {
			backend::matchmaker::lobby_runtime::ProxyProtocol::Http => {
				models::CloudVersionMatchmakerPortProtocol::Http
			}
			backend::matchmaker::lobby_runtime::ProxyProtocol::Https => {
				models::CloudVersionMatchmakerPortProtocol::Https
			}
			backend::matchmaker::lobby_runtime::ProxyProtocol::Tcp => {
				models::CloudVersionMatchmakerPortProtocol::Tcp
			}
			backend::matchmaker::lobby_runtime::ProxyProtocol::TcpTls => {
				models::CloudVersionMatchmakerPortProtocol::TcpTls
			}
			backend::matchmaker::lobby_runtime::ProxyProtocol::Udp => {
				models::CloudVersionMatchmakerPortProtocol::Udp
			}
		}
	}
}

impl ApiFrom<models::CloudVersionMatchmakerProxyKind>
	for backend::matchmaker::lobby_runtime::ProxyKind
{
	fn api_from(
		value: models::CloudVersionMatchmakerProxyKind,
	) -> backend::matchmaker::lobby_runtime::ProxyKind {
		match value {
			models::CloudVersionMatchmakerProxyKind::None => {
				backend::matchmaker::lobby_runtime::ProxyKind::None
			}
			models::CloudVersionMatchmakerProxyKind::GameGuard => {
				backend::matchmaker::lobby_runtime::ProxyKind::GameGuard
			}
		}
	}
}

impl ApiFrom<backend::matchmaker::lobby_runtime::ProxyKind>
	for models::CloudVersionMatchmakerProxyKind
{
	fn api_from(value: backend::matchmaker::lobby_runtime::ProxyKind) -> Self {
		match value {
			backend::matchmaker::lobby_runtime::ProxyKind::None => {
				models::CloudVersionMatchmakerProxyKind::None
			}
			backend::matchmaker::lobby_runtime::ProxyKind::GameGuard => {
				models::CloudVersionMatchmakerProxyKind::GameGuard
			}
		}
	}
}

impl ApiTryFrom<models::CloudVersionMatchmakerCaptcha> for backend::captcha::CaptchaConfig {
	type Error = GlobalError;

	fn api_try_from(value: models::CloudVersionMatchmakerCaptcha) -> GlobalResult<Self> {
		ensure_with!(
			value.requests_before_reverify >= 0,
			MATCHMAKER_INVALID_VERSION_CONFIG,
			error = "`requests_before_reverify` out of bounds"
		);
		ensure_with!(
			value.verification_ttl >= 0,
			MATCHMAKER_INVALID_VERSION_CONFIG,
			error = "`verification_ttl` out of bounds"
		);

		Ok(backend::captcha::CaptchaConfig {
			requests_before_reverify: value.requests_before_reverify.api_try_into()?,
			verification_ttl: value.verification_ttl,
			hcaptcha: value.hcaptcha.map(|x| (*x).api_into()),
			turnstile: value.turnstile.map(|x| (*x).api_into()),
		})
	}
}

impl ApiTryFrom<backend::captcha::CaptchaConfig> for models::CloudVersionMatchmakerCaptcha {
	type Error = GlobalError;

	fn api_try_from(value: backend::captcha::CaptchaConfig) -> GlobalResult<Self> {
		Ok(models::CloudVersionMatchmakerCaptcha {
			requests_before_reverify: value.requests_before_reverify.api_try_into()?,
			verification_ttl: value.verification_ttl,
			hcaptcha: value
				.hcaptcha
				.map(ApiTryInto::api_try_into)
				.transpose()?
				.map(Box::new),
			turnstile: value
				.turnstile
				.map(ApiTryInto::api_try_into)
				.transpose()?
				.map(Box::new),
		})
	}
}

impl ApiFrom<models::CloudVersionMatchmakerCaptchaHcaptcha>
	for backend::captcha::captcha_config::Hcaptcha
{
	fn api_from(
		value: models::CloudVersionMatchmakerCaptchaHcaptcha,
	) -> backend::captcha::captcha_config::Hcaptcha {
		backend::captcha::captcha_config::Hcaptcha {
			level: ApiInto::<backend::captcha::captcha_config::hcaptcha::Level>::api_into(
				value.level,
			) as i32,
		}
	}
}

impl ApiTryFrom<backend::captcha::captcha_config::Hcaptcha>
	for models::CloudVersionMatchmakerCaptchaHcaptcha
{
	type Error = GlobalError;

	fn api_try_from(value: backend::captcha::captcha_config::Hcaptcha) -> GlobalResult<Self> {
		Ok(models::CloudVersionMatchmakerCaptchaHcaptcha {
			level: unwrap!(backend::captcha::captcha_config::hcaptcha::Level::from_i32(
				value.level
			))
			.api_into(),
		})
	}
}

impl ApiFrom<models::CloudVersionMatchmakerCaptchaHcaptchaLevel>
	for backend::captcha::captcha_config::hcaptcha::Level
{
	fn api_from(
		value: models::CloudVersionMatchmakerCaptchaHcaptchaLevel,
	) -> backend::captcha::captcha_config::hcaptcha::Level {
		match value {
			models::CloudVersionMatchmakerCaptchaHcaptchaLevel::Easy => {
				backend::captcha::captcha_config::hcaptcha::Level::Easy
			}
			models::CloudVersionMatchmakerCaptchaHcaptchaLevel::Moderate => {
				backend::captcha::captcha_config::hcaptcha::Level::Moderate
			}
			models::CloudVersionMatchmakerCaptchaHcaptchaLevel::Difficult => {
				backend::captcha::captcha_config::hcaptcha::Level::Difficult
			}
			models::CloudVersionMatchmakerCaptchaHcaptchaLevel::AlwaysOn => {
				backend::captcha::captcha_config::hcaptcha::Level::AlwaysOn
			}
		}
	}
}

impl ApiFrom<backend::captcha::captcha_config::hcaptcha::Level>
	for models::CloudVersionMatchmakerCaptchaHcaptchaLevel
{
	fn api_from(value: backend::captcha::captcha_config::hcaptcha::Level) -> Self {
		match value {
			backend::captcha::captcha_config::hcaptcha::Level::Easy => {
				models::CloudVersionMatchmakerCaptchaHcaptchaLevel::Easy
			}
			backend::captcha::captcha_config::hcaptcha::Level::Moderate => {
				models::CloudVersionMatchmakerCaptchaHcaptchaLevel::Moderate
			}
			backend::captcha::captcha_config::hcaptcha::Level::Difficult => {
				models::CloudVersionMatchmakerCaptchaHcaptchaLevel::Difficult
			}
			backend::captcha::captcha_config::hcaptcha::Level::AlwaysOn => {
				models::CloudVersionMatchmakerCaptchaHcaptchaLevel::AlwaysOn
			}
		}
	}
}

impl ApiFrom<models::CloudVersionMatchmakerCaptchaTurnstile>
	for backend::captcha::captcha_config::Turnstile
{
	fn api_from(
		value: models::CloudVersionMatchmakerCaptchaTurnstile,
	) -> backend::captcha::captcha_config::Turnstile {
		backend::captcha::captcha_config::Turnstile {
			site_key: value.site_key,
			secret_key: value.secret_key,
		}
	}
}

impl ApiTryFrom<backend::captcha::captcha_config::Turnstile>
	for models::CloudVersionMatchmakerCaptchaTurnstile
{
	type Error = GlobalError;

	fn api_try_from(value: backend::captcha::captcha_config::Turnstile) -> GlobalResult<Self> {
		Ok(models::CloudVersionMatchmakerCaptchaTurnstile {
			site_key: value.site_key,
			secret_key: value.secret_key,
		})
	}
}
