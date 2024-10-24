use proto::backend::pkg::*;
use rivet_operation::prelude::*;

#[operation(name = "cloud-namespace-token-public-create")]
async fn handle(
	ctx: OperationContext<cloud::namespace_token_public_create::Request>,
) -> GlobalResult<cloud::namespace_token_public_create::Response> {
	let namespace_id = unwrap_ref!(ctx.namespace_id).as_uuid();

	let ns_res = op!([ctx] game_namespace_get {
		namespace_ids: vec![namespace_id.into()],
	})
	.await?;
	let ns_data = ns_res.namespaces.first();
	let ns_data = unwrap_ref!(ns_data, "namespace not found");

	let token_res = op!([ctx] token_create {
		issuer: Self::NAME.into(),
		token_config: Some(token::create::request::TokenConfig {
			// Make these tokens not expire
			ttl: util::duration::days(365 * 15),
		}),
		refresh_token_config: None,
		client: None,
		kind: Some(token::create::request::Kind::New(token::create::request::KindNew {
			entitlements: vec![
				proto::claims::Entitlement {
					kind: Some(
						proto::claims::entitlement::Kind::GameNamespacePublic(proto::claims::entitlement::GameNamespacePublic {
							namespace_id: Some(namespace_id.into())
						})
					)
				}
			],
		})),
		label: Some(format!("pub_{}", ns_data.name_id.replace('-', "_"))),
		..Default::default()
	})
	.await?;

	let token = unwrap_ref!(token_res.token);
	let token_session_id = unwrap_ref!(token_res.session_id).as_uuid();

	sql_execute!(
		[ctx]
		"
		INSERT INTO db_cloud.game_namespace_public_tokens (namespace_id, token_session_id)
		VALUES ($1, $2)
		",
		namespace_id,
		token_session_id,
	)
	.await?;

	Ok(cloud::namespace_token_public_create::Response {
		token: token.token.clone(),
	})
}
