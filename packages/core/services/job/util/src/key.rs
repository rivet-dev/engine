use uuid::Uuid;

/// HASH<run id, rivet.db.job.RunProxiedPort>
pub fn proxied_ports(region_id: Uuid) -> String {
	format!("{{global}}:job:region:{}:proxied_ports", region_id)
}
