locals {
	service_nsfw_api = lookup(var.services, "nsfw-api", {
		count = var.deploy_method_cluster ? 2 : 1
		resources = {
			cpu = 1000
			memory = 1024
		}
	})
}

resource "kubernetes_namespace" "nsfw_api" {
	count = var.nsfw_api_enabled ? 1 : 0

	metadata {
		name = "nsfw-api"
	}
}

resource "kubernetes_deployment" "nsfw_api" {
	count = var.nsfw_api_enabled ? 1 : 0
	depends_on = [null_resource.daemons, module.docker_auth]

	metadata {
		name = "nsfw-api"
		namespace = kubernetes_namespace.nsfw_api.0.metadata[0].name
	}

	spec {
		replicas = local.service_nsfw_api.count

		selector {
			match_labels = {
				"app.kubernetes.io/name" = "nsfw-api"
			}
		}

		template {
			metadata {
				labels = {
					"app.kubernetes.io/name" = "nsfw-api"
				}
			}

			spec {
				priority_class_name = kubernetes_priority_class.service_priority.metadata.0.name
				
				# MARK: Docker auth
				image_pull_secrets {
					name = "docker-auth"
				}

				container {
					image = "eugencepoi/nsfw_api@sha256:087d880e38b82e5cbee761bafd50e5093a40f813d3f0e77a8077f661cbcdb414"
					name = "nsfw-api"

					env {
					  name = "PORT"
					  value = 21900
					}

					port {
						name = "http"
						container_port = 21900
					}
					
					dynamic "resources" {
						for_each = var.limit_resources ? [0] : []

						content {
							limits = {
								cpu = "${local.service_nsfw_api.resources.cpu}m"
								memory = "${local.service_nsfw_api.resources.memory}Mi"
							}
						}
					}
				}
			}
		}
	}
}

resource "kubernetes_service" "nsfw_api" {
	count = var.nsfw_api_enabled ? 1 : 0

	metadata {
		name = "nsfw-api"
		namespace = kubernetes_namespace.nsfw_api.0.metadata[0].name
	}
	spec {
		selector = {
			"app.kubernetes.io/name" = kubernetes_deployment.nsfw_api.0.metadata.0.name
		}

		port {
			protocol = "TCP"
			port = 21900
			target_port = "http"
		}
	}
}
