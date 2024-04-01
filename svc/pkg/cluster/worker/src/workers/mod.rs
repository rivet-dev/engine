pub mod create;
pub mod datacenter_create;
pub mod datacenter_scale;
pub mod datacenter_taint;
pub mod datacenter_taint_complete;
pub mod datacenter_update;
pub mod nomad_node_drain_complete;
pub mod nomad_node_registered;
pub mod server_destroy;
pub mod server_dns_create;
pub mod server_dns_delete;
pub mod server_drain;
pub mod server_install;
pub mod server_install_complete;
pub mod server_provision;
pub mod server_undrain;

chirp_worker::workers![
	server_dns_delete,
	server_install_complete,
	datacenter_taint,
	datacenter_taint_complete,
	server_dns_create,
	nomad_node_drain_complete,
	datacenter_update,
	nomad_node_registered,
	datacenter_create,
	create,
	server_destroy,
	server_install,
	server_drain,
	server_provision,
	datacenter_scale,
	server_undrain,
];
