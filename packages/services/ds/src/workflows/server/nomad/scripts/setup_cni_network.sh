#!/usr/bin/env bash
set -euf -o pipefail

log() {
    local timestamp=$(date +"%Y-%m-%d %H:%M:%S.%3N")
    echo "[$timestamp] [setup_cni_network] $@"
}

# MARK: Generate CNI parameters
#
# See https://github.com/containernetworking/cni/blob/b62753aa2bfa365c1ceaff6f25774a8047c896b5/cnitool/cnitool.go#L31

# See Nomad capabilities equivalent:
# https://github.com/hashicorp/nomad/blob/a8f0f2612ef9d283ed903721f8453a0c0c3f51c5/client/allocrunner/networking_cni.go#L105C46-L105C46
#
# See supported args:
# https://github.com/containerd/go-cni/blob/6603d5bd8941d7f2026bb5627f6aa4ff434f859a/namespace_opts.go#L22
jq -c <<EOF > "$NOMAD_ALLOC_DIR/cni-cap-args.json"
{
	"portMappings": $(cat "$NOMAD_ALLOC_DIR/cni-port-mappings.json")
}
EOF

export CNI_PATH="/opt/cni/bin"
export NETCONFPATH="/opt/cni/config"
export CNI_IFNAME="eth0"
export CAP_ARGS=$(cat "$NOMAD_ALLOC_DIR/cni-cap-args.json")
log "CAP_ARGS: $CAP_ARGS"

# MARK: Create network
#
# See Nomad network creation:
# https://github.com/hashicorp/nomad/blob/a8f0f2612ef9d283ed903721f8453a0c0c3f51c5/client/allocrunner/network_manager_linux.go#L119

# Name of the network in /opt/cni/config/$NETWORK_NAME.conflist
NETWORK_NAME="rivet-job"

log "Creating network $CONTAINER_ID"
ip netns add "$CONTAINER_ID"

log "Adding network $NETWORK_NAME to namespace $NETNS_PATH"
cnitool add "$NETWORK_NAME" "$NETNS_PATH" > $NOMAD_ALLOC_DIR/cni.json

log "Finished setting up CNI network"

