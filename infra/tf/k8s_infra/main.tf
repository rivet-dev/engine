terraform {
	required_providers {
		# TODO Revert to gavinbunney/kubectl once https://github.com/gavinbunney/terraform-provider-kubectl/issues/270 is resolved
		kubectl = {
			source = "alekc/kubectl"
			version = ">= 2.0.2"
		}
	}
}

locals {
	entrypoints = var.dns_enabled ? {
		"web" = {}
		"websecure" = {
			tls = {
				secretName = "ingress-tls-cloudflare-cert"
				options = {
					name = "ingress-cloudflare",
					namespace = "traefik"
				}

			}
		}
	} : {
		"web" = {}
	}
}
