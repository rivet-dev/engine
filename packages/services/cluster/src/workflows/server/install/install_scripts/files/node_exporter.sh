# https://github.com/prometheus/node_exporter/releases
version="1.6.1"

if ! id -u "node_exporter" &>/dev/null; then
	useradd -r -s /bin/false node_exporter
fi

# Download and install node_exporter
mkdir -p /opt/node_exporter-$version/
wget -O /tmp/node_exporter.tar.gz https://github.com/prometheus/node_exporter/releases/download/v$version/node_exporter-$version.linux-amd64.tar.gz
tar -zxvf /tmp/node_exporter.tar.gz -C /opt/node_exporter-$version/ --strip-components=1
install -o node_exporter -g node_exporter /opt/node_exporter-$version/node_exporter /usr/bin/

# TODO: Verify hash

# Check version
if [[ "$(node_exporter --version)" != *"$version"* ]]; then
	echo "Version check failed."
	exit 1
fi

# Create systemd service file
cat << 'EOF' > /etc/systemd/system/node_exporter.service
[Unit]
Description=Node Exporter
Requires=network-online.target
After=network-online.target

[Service]
User=node_exporter
Group=node_exporter
Type=simple
# Reduce cardinality
ExecStart=/usr/bin/node_exporter --collector.disable-defaults --collector.cpu --collector.netdev --collector.conntrack --collector.meminfo --collector.filesystem --collector.filesystem.mount-points-exclude=^/opt/nomad/ --collector.netstat --collector.sockstat --collector.tcpstat --collector.network_route --collector.arp --collector.filefd --collector.interrupts --collector.softirqs --collector.processes
Restart=always
RestartSec=2

[Install]
WantedBy=multi-user.target
EOF

# Start and enable node_exporter service
systemctl daemon-reload
systemctl enable node_exporter
systemctl start node_exporter

