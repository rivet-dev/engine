pub use aws_sdk_s3;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("env var: {0}")]
	VarError(#[from] std::env::VarError),
	#[error("invalid uri: {0}")]
	InvalidEndpoint(#[from] aws_smithy_http::endpoint::error::InvalidEndpointError),
	#[error("lookup host: {0}")]
	LookupHost(std::io::Error),
	#[error("unresolved host")]
	UnresolvedHost,
	#[error("unknown provider: {0}")]
	UnknownProvider(String),
}

/// How to access the S3 service.
pub enum EndpointKind {
	/// Used for making calls within the core cluster using private DNS.
	///
	/// This should be used for all API calls.
	Internal,

	/// Used for making calls within the cluster, but without access to the internal DNS server. This will
	/// resolve the IP address on the machine building the presigned request.
	///
	/// Should be used sparingly, incredibly hacky.
	InternalResolved,

	/// Used for making calls from outside of the cluster.
	///
	/// This should be used for all public presigned requests.
	External,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
	Minio,
	Backblaze,
	Aws,
}

impl Provider {
	pub fn default() -> Result<Self, Error> {
		Self::from_str(&std::env::var("S3_DEFAULT_PROVIDER")?)
	}

	pub fn from_str(s: &str) -> Result<Self, Error> {
		match s {
			"minio" => Ok(Provider::Minio),
			"backblaze" => Ok(Provider::Backblaze),
			"aws" => Ok(Provider::Aws),
			_ => Err(Error::UnknownProvider(s.to_string())),
		}
	}

	pub fn as_str(&self) -> &'static str {
		match self {
			Provider::Minio => "minio",
			Provider::Backblaze => "backblaze",
			Provider::Aws => "aws",
		}
	}
}

#[derive(Clone)]
pub struct Client {
	bucket: String,
	client: aws_sdk_s3::Client,
}

impl std::ops::Deref for Client {
	type Target = aws_sdk_s3::Client;

	fn deref(&self) -> &aws_sdk_s3::Client {
		&self.client
	}
}

impl Client {
	pub fn new(
		bucket: &str,
		endpoint: &str,
		region: &str,
		access_key_id: &str,
		secret_access_key: &str,
	) -> Result<Self, Error> {
		let config = aws_sdk_s3::Config::builder()
			.region(aws_sdk_s3::Region::new(region.to_owned()))
			.endpoint_resolver(aws_sdk_s3::Endpoint::immutable(endpoint)?)
			.credentials_provider(aws_sdk_s3::Credentials::new(
				access_key_id,
				secret_access_key,
				None,
				None,
				"Static",
			))
			// .sleep_impl(Arc::new(aws_smithy_async::rt::sleep::TokioSleep::new()))
			.build();
		let client = aws_sdk_s3::Client::from_conf(config);

		Ok(Client {
			bucket: bucket.to_owned(),
			client,
		})
	}

	pub async fn from_env(svc_name: &str) -> Result<Self, Error> {
		Self::from_env_opt(svc_name, Provider::default()?, EndpointKind::Internal).await
	}

	pub async fn from_env_with_provider(svc_name: &str, provider: Provider) -> Result<Self, Error> {
		Self::from_env_opt(svc_name, provider, EndpointKind::Internal).await
	}

	pub async fn from_env_opt(
		svc_name: &str,
		provider: Provider,
		endpoint_kind: EndpointKind,
	) -> Result<Self, Error> {
		let svc_screaming = svc_name.to_uppercase().replace("-", "_");

		let provider_upper = provider.as_str().to_uppercase();

		let bucket = std::env::var(format!("S3_{}_BUCKET_{}", provider_upper, svc_screaming))?;
		let region = std::env::var(format!("S3_{}_REGION_{}", provider_upper, svc_screaming))?;
		let access_key_id = std::env::var(format!(
			"S3_{}_ACCESS_KEY_ID_{}",
			provider_upper, svc_screaming
		))?;
		let secret_access_key = std::env::var(format!(
			"S3_{}_SECRET_ACCESS_KEY_{}",
			provider_upper, svc_screaming
		))?;

		let endpoint = match endpoint_kind {
			EndpointKind::Internal => std::env::var(format!(
				"S3_{}_ENDPOINT_INTERNAL_{}",
				provider_upper, svc_screaming
			))?,
			EndpointKind::InternalResolved => {
				let mut endpoint = std::env::var(format!(
					"S3_{}_ENDPOINT_INTERNAL_{}",
					provider_upper, svc_screaming
				))?;

				// HACK: Resolve Minio DNS address to schedule the job with. We
				// do this since the job servers don't have the internal DNS servers
				// to resolve the Minio endpoint.
				//
				// This has issues if there's a race condition with changing the
				// Minio address.
				//
				// We can't resolve the presigned URL, since the host's presigned
				// host is part of the signature.
				const MINIO_K8S_HOST: &str = "minio.minio.svc.cluster.local:9200";
				if endpoint.contains(MINIO_K8S_HOST) {
					tracing::info!(host = %MINIO_K8S_HOST, "looking up dns");

					// Resolve IP
					let mut hosts = tokio::net::lookup_host(MINIO_K8S_HOST)
						.await
						.map_err(Error::LookupHost)?;
					let Some(host) = hosts.next() else {
						return Err(Error::UnresolvedHost);
					};

					// Substitute endpoint with IP
					endpoint = endpoint.replace(MINIO_K8S_HOST, &host.to_string());
				}

				endpoint
			}
			EndpointKind::External => std::env::var(format!(
				"S3_{}_ENDPOINT_EXTERNAL_{}",
				provider_upper, svc_screaming
			))?,
		};

		Self::new(
			&bucket,
			&endpoint,
			&region,
			&access_key_id,
			&secret_access_key,
		)
	}

	pub fn bucket(&self) -> &str {
		&self.bucket
	}
}

pub fn s3_provider_active(svc_name: &str, provider: Provider) -> bool {
	let svc_screaming = svc_name.to_uppercase().replace("-", "_");
	let provider_upper = provider.as_str().to_uppercase();

	std::env::var(format!("S3_{}_BUCKET_{}", provider_upper, svc_screaming)).is_ok()
} 

pub fn s3_region(svc_name: &str, provider: Provider) -> Result<String, Error> {
	let svc_screaming = svc_name.to_uppercase().replace("-", "_");
	let provider_upper = provider.as_str().to_uppercase();

	std::env::var(format!("S3_{}_REGION_{}", provider_upper, svc_screaming)).map_err(Into::into)
} 

pub fn s3_credentials(svc_name: &str, provider: Provider) -> Result<(String, String), Error> {
	let svc_screaming = svc_name.to_uppercase().replace("-", "_");
	let provider_upper = provider.as_str().to_uppercase();

	let access_key_id = std::env::var(format!(
		"S3_{}_ACCESS_KEY_ID_{}",
		provider_upper, svc_screaming
	))?;
	let secret_access_key = std::env::var(format!(
		"S3_{}_SECRET_ACCESS_KEY_{}",
		provider_upper, svc_screaming
	))?;

	Ok((access_key_id, secret_access_key))
}

pub fn s3_endpoint_external(svc_name: &str, provider: Provider) -> Result<String, Error> {
	let svc_screaming = svc_name.to_uppercase().replace("-", "_");
	let provider_upper = provider.as_str().to_uppercase();

	std::env::var(format!(
		"S3_{}_ENDPOINT_EXTERNAL_{}",
		provider_upper, svc_screaming
	)).map_err(Into::into)
}
