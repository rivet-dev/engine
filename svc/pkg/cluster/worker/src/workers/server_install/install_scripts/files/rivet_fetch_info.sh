PUBLIC_IP=$(ip -4 route get 1.0.0.0 | awk '{print $7; exit}')

# Get server info from rivet
response=$(
	curl -f \
		-H "Authorization: Bearer __SERVER_TOKEN__" \
		"https://__DOMAIN_MAIN_API__/provision/servers/$PUBLIC_IP/info"
)

# Fetch data
name=$(echo $response | jq -r '.name')
server_id=$(echo $response | jq -r '.server_id')
datacenter_id=$(echo $response | jq -r '.datacenter_id')
cluster_id=$(echo $response | jq -r '.cluster_id')
vlan_ip=$(echo $response | jq -r '.vlan_ip')

# Template initialize script
initialize_script="/usr/bin/rivet_initialize.sh"
sed -i "s/__NODE_NAME__/$name/g" $initialize_script
sed -i "s/__SERVER_ID__/$server_id/g" $initialize_script
sed -i "s/__DATACENTER_ID__/$datacenter_id/g" $initialize_script
sed -i "s/__CLUSTER_ID__/$cluster_id/g" $initialize_script
sed -i "s/__VLAN_IP__/$vlan_ip/g" $initialize_script

# Run initialize script
"$initialize_script"
