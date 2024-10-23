#!/usr/bin/env bash
set -euf -o pipefail

# FERN_NO_VERSION_REDIRECTION=true FERN_DISABLE_TELEMETRY=true fern generate --local --group $FERN_GROUP --log-level debug

# To install fern, first clone the repo and check out the branch
# $ git clone https://github.com/rivet-gg/fern
# $ cd fern
# $ git checkout max/remove-headers
#
# Then, follow the instructions in SETUP.md and CONTRIBUTING.md to compile fern
# $ yarn
# $ yarn compile
# $ yarn dist:cli:dev
# $ docker image ls | grep fern
# $ docker builder prune
# $ yarn workspace @fern-typescript/sdk-generator-cli run dockerTagVersion:browser 999.999.999
#
# If needed:
# $ yarn workspace @fern-api/openapi-generator run dockerTagVersion 999.999.999
#
# Finally, run this with the path to the fern repo, say:
# $ FERN_REPO_PATH=~/fern ./oss/scripts/fern/gen.sh

set +u
if [ -z "$FERN_REPO_PATH" ]; then
	echo 'Please clone and build https://github.com/rivet-gg/fern/tree/max/remove-headers and set $FERN_REPO_PATH. This is a workaround until https://github.com/fern-api/fern/pull/2551 is resolved.'
	exit 1
fi
set -u

# Generate typescript SDK docker image
if ! docker images | grep -w "^fernapi/fern-typescript-browser-sdk.*999\.999\.999" > /dev/null; then
	echo "Generating TypeScript SDK"
	(cd "$FERN_REPO_PATH" && nix-shell -p yarn --run 'yarn workspace @fern-typescript/sdk-generator-cli dockerTagVersion:browser 999.999.999')
fi

# Generate Fern libraries
echo "Using Fern from $FERN_REPO_PATH"
FERN_NO_VERSION_REDIRECTION=true FERN_DISABLE_TELEMETRY=true node "$FERN_REPO_PATH/packages/cli/cli/dist/dev/cli.cjs" generate --local --group $FERN_GROUP --log-level debug

# Add missing deps
(cd sdks/$FERN_GROUP/typescript && nix-shell -p jq --run 'jq ".devDependencies[\"@types/node-fetch\"] = \"2.6.11\"" package.json > package.json.tmp && mv package.json.tmp package.json')

# Build libraries
#
# See https://github.com/fern-api/fern-typescript/blob/3b1c33781bbd726cee26a21c1ff3464eeae70cad/README.md?plain=1#L379
(cd sdks/$FERN_GROUP/typescript && yarn install && yarn pack -f archive.tgz) &

# Generate OpenAPI clients
(./scripts/openapi/gen_spec_compat.sh && ./scripts/openapi/gen_rust.sh) &

wait

