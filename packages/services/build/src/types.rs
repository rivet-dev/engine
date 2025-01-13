use chirp_workflow::prelude::*;
use rivet_api::models;
use rivet_convert::ApiFrom;
use std::collections::HashMap;
use strum::FromRepr;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq, Eq, FromRepr)]
pub enum BuildKind {
	DockerImage = 0,
	OciBundle = 1,
	JavaScript = 2,
}

impl ApiFrom<models::ActorBuildKind> for BuildKind {
	fn api_from(value: models::ActorBuildKind) -> BuildKind {
		match value {
			models::ActorBuildKind::DockerImage => BuildKind::DockerImage,
			models::ActorBuildKind::OciBundle => BuildKind::OciBundle,
			models::ActorBuildKind::Javascript => BuildKind::JavaScript,
		}
	}
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq, Eq, FromRepr)]
pub enum BuildCompression {
	None = 0,
	Lz4 = 1,
}

impl ApiFrom<models::ActorBuildCompression> for BuildCompression {
	fn api_from(value: models::ActorBuildCompression) -> BuildCompression {
		match value {
			models::ActorBuildCompression::None => BuildCompression::None,
			models::ActorBuildCompression::Lz4 => BuildCompression::Lz4,
		}
	}
}

#[derive(Debug)]
pub struct Build {
	pub build_id: Uuid,
	pub game_id: Option<Uuid>,
	pub env_id: Option<Uuid>,
	pub upload_id: Uuid,
	pub display_name: String,
	pub image_tag: String,
	pub create_ts: i64,
	pub kind: BuildKind,
	pub compression: BuildCompression,
	pub tags: HashMap<String, String>,
}

// TODO: Move to upload pkg when its converted to new ops
pub mod upload {
	use std::convert::TryInto;

	use chirp_workflow::prelude::*;
	use rivet_api::models;
	use rivet_convert::ApiTryFrom;
	use rivet_operation::prelude::proto::backend;

	#[derive(Debug)]
	pub struct PrepareFile {
		pub path: String,
		pub mime: Option<String>,
		pub content_length: u64,
		pub multipart: bool,
	}

	impl ApiTryFrom<models::UploadPrepareFile> for PrepareFile {
		type Error = GlobalError;

		fn api_try_from(value: models::UploadPrepareFile) -> GlobalResult<Self> {
			Ok(PrepareFile {
				path: value.path,
				mime: value.content_type,
				content_length: value.content_length.try_into()?,
				multipart: false,
			})
		}
	}

	#[derive(Debug)]
	pub struct PresignedUploadRequest {
		pub path: String,
		pub url: String,
		pub part_number: u32,
		pub byte_offset: u64,
		pub content_length: u64,
	}

	impl From<backend::upload::PresignedUploadRequest> for PresignedUploadRequest {
		fn from(value: backend::upload::PresignedUploadRequest) -> Self {
			PresignedUploadRequest {
				path: value.path,
				url: value.url,
				part_number: value.part_number,
				byte_offset: value.byte_offset,
				content_length: value.content_length,
			}
		}
	}

	impl ApiTryFrom<PresignedUploadRequest> for models::UploadPresignedRequest {
		type Error = GlobalError;

		fn api_try_from(value: PresignedUploadRequest) -> GlobalResult<Self> {
			Ok(models::UploadPresignedRequest {
				path: value.path,
				url: value.url,
				byte_offset: value.byte_offset.try_into()?,
				content_length: value.content_length.try_into()?,
			})
		}
	}
}
