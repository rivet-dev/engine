# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Calendar Versioning](https://calver.org/).

## [24.3.0](https://github.com/rivet-gg/rivet/compare/v24.2.0...v24.3.0) (2024-03-01)


### Features

* **bolt:** add region filter to ssh command ([#537](https://github.com/rivet-gg/rivet/issues/537)) ([af274a8](https://github.com/rivet-gg/rivet/commit/af274a8e99666e24f3f289b389246347fbb9ae1d))
* expose nomad dashboard via cloudflare tunnels ([#543](https://github.com/rivet-gg/rivet/issues/543)) ([3a574c0](https://github.com/rivet-gg/rivet/commit/3a574c03dfad3d7e0bb8a733576b1220608f2ea1))
* **Main:** Added Devcontainer files ([9bb97db](https://github.com/rivet-gg/rivet/commit/9bb97db1e3b211830eada237eca3b6fa210ba7b8))
* **mm:** add config to opt-in individual games for host networking & root containers ([#549](https://github.com/rivet-gg/rivet/issues/549)) ([be9ddd6](https://github.com/rivet-gg/rivet/commit/be9ddd6328a06bf3057d78ed94d9bd7c66c41284))


### Bug Fixes

* add checksum annotations to cloudflared deployment ([#542](https://github.com/rivet-gg/rivet/issues/542)) ([f2d847b](https://github.com/rivet-gg/rivet/commit/f2d847be17aa7b23d060292ec0aba6c213717a37))
* **bolt:** clarify 1password service token warning ([#541](https://github.com/rivet-gg/rivet/issues/541)) ([eb2e7d5](https://github.com/rivet-gg/rivet/commit/eb2e7d58c5b8f6e07bfa7740d15ae5da25f68987))
* correct hcaptcha length ([#548](https://github.com/rivet-gg/rivet/issues/548)) ([748aaa8](https://github.com/rivet-gg/rivet/commit/748aaa8d38a724b5f5f3bac0d7993cb7ace50045))
* inaccessible admin routes ([#555](https://github.com/rivet-gg/rivet/issues/555)) ([9896b09](https://github.com/rivet-gg/rivet/commit/9896b09821d86f01cf6729841764195eabb6b3dd))
* revert to redis-rs v0.23.3 with panic patch ([#552](https://github.com/rivet-gg/rivet/issues/552)) ([3780eaa](https://github.com/rivet-gg/rivet/commit/3780eaa2fa6fa5f2840411193e617b9b77984b43))
* updated docs error url ([#544](https://github.com/rivet-gg/rivet/issues/544)) ([7099658](https://github.com/rivet-gg/rivet/commit/70996584bee4678d3d42afc49ed3ed3053b9c44c))

## [24.2.0] - 2024-02-22

### Added

-   **Infra** Added Better Uptime monitor
-   **Bolt** Add Docker `RUN` cache to distributed deploys to improve deploy speeds
-   **Infra** Prometheus VPA
-   **Infra** Apache Traffic Server VPA
-   **api-cloud** Admins can view all teams & games in a cluster
-   Added automatic deploy CI for staging
-   **Infra** Added compactor and GC to Loki
-   **api-status** Test individual Game Guard nodes to ensure all nodes have the correct configuration
-   Generate separate SDKs for `runtime` (lightweight, essentials for running a game) and `full` (heavy, includes cloud APIs)
-   Metrics for cache operations as well as a Grafana dashboard
-   **Bolt** Added namespace config and secrets sync with `bolt config pull` and `bolt config push` via 1Password
-   `GROUP_DEACTIVATED` error now shows reasons for deactivation. Added docs for deactivation reasons
-   `/health/essential` endpoint to test connectivity to all essential services
-   Added error when trying to deploy a distributed cluster on a non-linux-x86 machine (not supported)

### Changed

-   **api-status** More comprehensive status check that both creates a lobby & connects to it
-   More details in `CLAIMS_MISSING_ENTITLEMENT` error
-   **API** Added 120s timeout to reading request body and writing response to all requests going through Traefik
-   **Infra** Update Promtail logs to match k8s semantics
-   **Infra** Added `Cache-Control: no-cache` to 400 responses from CDN
-   **[BREAKING]** **Infra** Removed config-less hCaptcha. You are now required to provide a site key and
    secret key for the hCaptcha config in your game version matchmaker config for all future versions (old
    version will remain operational using our own hCaptcha site key).
-   **Internal** Updated source hash calculation to use `git diff` and `git rev-parse HEAD`
-   **API** Removed `x-fern-*` headers from generated TypeScript clients
-   Implemented liveness probe to check connectivity to essential services
-   Remove public-facing health check endpoints
-   **API** Removed ability to choose a name id when creating a game. One will be generated based on the given display name
-   **Infra** Reduced allocated cache size on ATS nodes to prevent disks exhaustion

### Fixed

-   **Bolt** Prompt prod won't prompt if does not have user control
-   **Bolt** Exclude copying bloat from `infra/tf/` to distributed Docker builds
-   Invalid JWT tokens now return explicit `TOKEN_INVALID` error instead of 500
-   **Infra** Remove debug logging from traefik-tunnel
-   Game lobby logs now ship even when the lobby fails immediately
-   Fixed `CLAIMS_MISSING_ENTITLEMENT` not formatting correctly (reason given was `?`)
-   Added role ARN to exec commands in `k8s-cluster-aws` tf provider to properly authenticate
-   Change email attached to Stripe on group ownership change
-   Enable `keep-alive` on `redis` crate
-   Update `redis` crate to mitigate panic on connection failure during `AUTH`
-   Wrong grace period for GG config to update after `mm::msg::lobby_ready`

### Security

-   Resolve [RUSTSEC-2024-0003](https://rustsec.org/advisories/RUSTSEC-2024-0003)

## [24.1.0] - 2024-01-23

### Added

-   **Infra** New `job-runner` crate responsible for managing the OCI bundle runtime & log shipping on the machine
-   **Infra** Jobs now log an explicit rate message when logs are rate limited & truncated
-   **Infra** `infra-artifacts` Terraform plan & S3 bucket used for automating building & uploading internal binaries, etc.
-   **Infra** Aiven Redis provider
-   **Bolt** `bolt secret set <path> <value>` command
-   **Bolt** `bolt.confirm_commands` to namespace to confirm before running commands on a namespace
-   `watch-requests` load test
-   `mm-sustain` load test
-   **Infra** Automatic server provisioning system ([Read more](/docs/packages/cluster/SERVER_PROVISIONING.md)).

### Changed

-   **Matchmaker** Allow excluding `matchmaker.regions` in order to enable all regions
-   **Matchmaker** Lowered internal overhead of log shipping for lobbies
-   **Matchmaker** Game mode names are now more lenient to include capital letters & underscores
-   **API** Return `API_REQUEST_TIMEOUT` error after 50s (see `docs/infrastructure/TIMEOUTS.md` for context)
-   **API** Move generated client APIs to sdks/
-   **API** Lower long poll timeout from 60s -> 40s
-   **Bolt** Moved additional project roots to Bolt.toml
-   **types** Support multiple project roots for reusing Protobuf types
-   **Infra** Switch from AWS ELB to NLB to work around surge queue length limitation
-   **Infra** Loki resources are now configurable
-   **pools** Allow infinite Redis reconnection attempts
-   **pools** Set Redis client names
-   **pools** Ping Redis every 15 seconds
-   **pools** Enable `test_before_acquire` on SQLx
-   **pools** Decrease SQLx `idle_timeout` to 3 minutes
-   **pools** Set ClickHouse `idle_timeout` to 15 seconds
-   **api-helper** Box path futures for faster compile times
-   Upgrade `async-nats`
-   `test-mm-lobby-echo` now handles `SIGTERM` and exits immediately, allows for less resource consumption while testing lobbies
-   **mm** Dynamically sleep based on lobby's `create_ts` for Treafik config to update
-   **Infra** Update Traefik tunnel client & server to v3.0.0-beta5
-   **Infra** Update Traefik load balancer to v2.10.7

### Security

-   Resolve [RUSTSEC-2023-0044](https://rustsec.org/advisories/RUSTSEC-2023-0074)

### Fixed

-   **Infra** runc rootfs is now a writable file system
-   **Matchmaker** Logs not shipping if lobby exits immediately
-   **Matchmaker** Returning `lnd-atl` instead of `dev-lcl` as the mocked mocked region ID in the region list
-   **API** 520 error when long polling
-   **api-cloud** Returning wrong domain for `domains.cdn`
-   **Infra** Fix Prometheus storage retention conversion between mebibytes and megabytes
-   **Infra** Fix typo in Game Guard Traefik config not exposing API endpoint
-   **Infra** Kill signal for servers was `SIGINT` instead of `SIGTERM`
-   **Infra** NATS cluster not getting enabled
-   **Infra** Redis Kubernetes error when using non-Kubernetes provider
-   **api-helper** Remove excess logging
-   `user_identity.identities` not getting purged on create & delete
-   **Bolt** Error when applying Terraform when a plan is no longer required
-   **api-helper** Instrument path futures
-   **Infra** CNI ports not being removed from the `nat` iptable, therefore occasionally causing failed connections
-   **Infra** Disable `nativeLB` for Traefik tunnel
-   **Infra** Update default Nomad storage to 64Gi
-   **Infra** Tunnel now exposes each Nomad server individually so the Nomad client can handle failover natively instead of relying on Traefik
-   **Infra** Traefik tunnel not respecting configured replicas
-   **Bolt** ClickHouse password generation now includes required special characters

## [23.2.0-rc.1] - 2023-12-01

### Added

-   **Infra** Lobby tagging system for filtering lobbies in `/find`
-   **Infra** Dynamically configurable max player count in `/find` and `/create`
-   **Bolt** Added `bolt admin login` to allow for logging in without an email provider setup. Automatically turns the user into an admin for immediate access to the developer dashboard.
-   **Bolt** Fixed `bolt db migrate create`
-   **Infra** Added `user-admin-set` service for creating an admin user
-   **api-cloud** `/bootstrap` properties for `access` and `login_methods`

### Changed

-   **Bolt** Removed `bolt admin team-dev create`. You can use `bolt admin login` and the hub to create a new dev team
-   **Infra** Turnstile `CAPTCHA_CAPTCHA_REQUIRED` responses now include a site key
-   **Infra** Turnstile is no longer configurable by domain (instead configured by Turnstile itself)
-   **Infra** Job log aggregating to use Vector under the hood to insert directly into ClickHouse
-   **Matchmaker** Players automatically remove after extended periods of time to account for network failures

### Fixed

-   **Infra** Job logs occasionally returning duplicate log lines
-   **Matchmaker** /list returning no lobbies unless `include_state` query parameter is `true`
-   **Matchmaker** Players remove correctly when the player fails to be inserted into the Cockroach database and only exists in Redis
-   **Chirp** `tail_all` default timeouts are now lower than `api-helper` timeout
-   **api-kv** Batch operation timeouts are now lower than `api-helper` timeout

## [23.1.0] - 2023-10-30

### Added

-   **Bolt** Development cluster can now be booted without any external services (i.e. no Linode & Cloudflare account required, does not require LetsEncrypt cert)
-   **Infra** Autoscale non-singleton services based on CPU & memory
-   **Infra** Support for running ClickHouse on ClickHouse Cloud
-   **Infra** Support for running CockroachDB on Cockroach Cloud
-   **Infra** Support for running Redis on AWS ElastiCache & MemoryDB
-   **Infra** Dynamically provisioned core cluster using Karpenter
-   **Infra** Dual-stack CNI configuration for game containers
-   **Infra** job iptables firewall to job pool that whitelists inbound traffic from Game Guard to the container
-   **Infra** job iptables rules to configure minimize delay TOS for traffic without a TOS
-   **Infra** job iptables rules to configure maximize throughput TOS for traffic from ATS
-   **Infra** job Linux traffic control filters to prioritize game traffic over other background traffic
-   **Infra** Prewarm the Traffic Server cache when a game version is published for faster cold start times on the first booted lobby in each region
-   **Infra** Envoy Maglev load balancing for traffic to edge Traffic Server instances to maximize cache hits
-   **Bolt** Timeout for tests
-   **Bolt** New summary view of test progress
-   **Bolt** `config show` command
-   **Bolt** `ssh pool --all <COMMAND>` command
-   **Bolt** Validation that the correct pools exist in th enamespace
-   **Bolt** Validation that the matchmaker delivery method is configured correctly depending on wether ATS servers exist
-   **Dev** Bolt automatically builds with Nix shell
-   **Bolt** `--no-purge` flag to `test` to prevent purging Nomad jobs
-   **Matchmaker** Expose hardware metrics to container with `RIVET_CPU`, `RIVET_MEMORY`, and `RIVET_MEMORY_OVERSUBSCRIBE`
-   **api-cloud** `GET /cloud/bootstrapp` to provide initial config data to the hub
-   **api-cloud** Dynamically send Turnstile site key to hub
-   **Infra** Rate limit on creating new SQL connections to prevent stampeding connections

### Changed

-   Cleaned up onboarding experience for open source users, see _docs/getting_started/DEVELOPMENT.md_
-   **Infra** Moved default API routes from `{service}.api.rivet.gg/v1` to `api.rivet.gg/{service}`
-   **Infra** Removed version flat from API request paths
-   **Bolt** Tests are built in batch and binaries are ran in parallel in order to speed up test times
-   **Bolt** Run tests inside of Kubernetes pod inside cluster, removing the need for port forwarding for tests
-   **Bolt** Remove `disable_cargo_workspace` flag since it is seldom used
-   **Bolt** Remove `skip_dependencies`, `force_build`, and `skip_generate` on `bolt up` and `bolt test` commands that are no longer relevant
-   **api-route** Split up routes in to `/traefik/config/core` and `/traefik/config/game-guard`
-   **Imagor** CORS now mirror the default CORS configured for S3
-   **Dev** `git lfs install` automatically runs in `shellHook`
-   **Dev** Removed `setup.sh` in lieu of `shellHook`
-   Replaced `cdn.rivet.gg` domains with presigned requests directly to the S3 provider
-   **api-matchmaker** Gracefully disable automatic region selection when coords not obtainable
-   **Infra** Disabling DNS uses `X-Forwarded-For` header for the client IP
-   **Infra** Pool connections are now created in parallel for faster tests & service start times
-   **Infra** Connections from edge <-> core services are now done over mTLS with Treafik instead of cloudflared
-   **Infra** ClickHouse database connections now use TLS
-   **Infra** CockroachDB database connections now use TLS
-   **Infra** Redis database connections now use TLS
-   **Infra** Redis now uses Redis Cluster for everything
-   **Infra** Cloudflare certificate authority from DigitCert to Lets Encrypt
-   **Infra** Removed 1.1.1.1 & 1.0.0.1 as resolvers from Nomad jobs due to reliability issues
-   **Infra** Added IPv6 DNS resolvers to Nomad jobs
-   **Infra** CNI network for jobs from bridge to ptp for isolation & performance
-   **Infra** Remove requirement of `Content-Type: application/x-tar` for builds because of new compression types
-   **Matchmaker** Expose API origin to `RIVET_API_ENDPOINT` env var to lobby containers
-   **[BREAKING]** **Infra** Removed undocumented environment variables exposed by Nomad (i.e. anything prefixed with `NOMAD_`)

### Fixed

-   `LC_ALL: cannot change locale` error from glibc
-   **Dev** Bolt uses `write_if_different` for auto-generated files to prevent cache purging

## [23.1.0-rc4] - 2023-09-02

### Changed

-   Revert Fern TypeScript generator to 0.5.6 to fix bundled export

## [23.1.0-rc3] - 2023-09-02

### Changed

-   Don't publish internal Fern package on tag to prevent duplicate pushes

## [23.1.0-rc2] - 2023-09-02

### Changed

-   Update to Fern 0.15.0-rc7
-   Update Fern TypeScript, Java, and Go generators

## [23.1.0-rc1] - 2023-09-02

### Added

-   **Matchmaker** Support custom lobbies
-   **Matchmaker** Support lobby state
-   **Matchmaker** Support external verification
-   **Library** Support Java library
-   **Library** Support Go library
-   **Cloud** Support multipart uploads for builds
-   **Infra** Support configuring multiple S3 providers
-   **Infra** Support multipart uploads
-   **Infra** Replace Promtail-based log shipping with native Loki Docker driver
-   **Infra** Local Traefik Cloudflare proxy daemon for connecting to Cloudflare Access services
-   **Infra** Upload service builds to default S3 provider instead of hardcoded bucket
-   **Infra** Enable Apache Traffic Server pull-through cache for Docker builds
-   **Bolt** Support for connecting to Redis databases with `bolt redis sh`
-   **Bolt** Confirmation before running any command in the production namespace
-   **Bolt** `--start-at` flag for all infra commands
-   **Bolt** Explicit database dependencies in services to reduce excess database pools

### Changed

-   **Infra** Update CNI plugins to 1.3.0
-   **Infra** Update ClickHouse to 23.7.2.25
-   **Infra** Update Cockroach to 23.1.7
-   **Infra** Update Consul Exporter to 1.9.0
-   **Infra** Update Consul to 1.16.0
-   **Infra** Update Imagor to 1.4.7
-   **Infra** Update NATS server to 2.9.20
-   **Infra** Update Node Exporter server to 1.6.0
-   **Infra** Update Nomad to 1.6.0
-   **Infra** Update Prometheus server to 2.46.0
-   **Infra** Update Redis Exporter to 1.52.0
-   **Infra** Update Redis to 7.0.12
-   **Infra** Update Traefik to 2.10.4
-   **Bolt** PostHog events are now captured in a background task
-   **Bolt** Auto-install rsync on Salt Master
-   **Bolt** Recursively add dependencies from overridden services when using additional roots
-   **KV** Significantly rate limit of all endpoints

### Security

-   Resolve [RUSTSEC-2023-0044](https://rustsec.org/advisories/RUSTSEC-2023-0044)
-   Resolve [RUSTSEC-2022-0093](https://rustsec.org/advisories/RUSTSEC-2022-0093)
-   Resolve [RUSTSEC-2023-0053](https://rustsec.org/advisories/RUSTSEC-2023-0053)

### Fixed

-   **Portal** Skip captcha if no Turnstile key provided
-   **Infra** Missing dpenedency on mounting volume before setting permissions of /var/\* for Cockroach, ClickHouse, Prometheus, and Traffic Server
-   **Chrip** Empty message parameters now have placeholder so NATS doesn't throw an error
-   **Chrip** Messages with no parameters no longer have a trailing dot
-   **Bolt** Correctly resolve project root when building services natively
-   **Bolt** Correctly determine executable path for `ExecServiceDriver::UploadedBinaryArtifact` with different Cargo names
