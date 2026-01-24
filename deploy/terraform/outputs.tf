# Terraform outputs for Fen deployment

# Cluster Information
output "cluster_name" {
  description = "Name of the Kubernetes cluster"
  value       = local.cluster_name
}

output "cluster_endpoint" {
  description = "Kubernetes API endpoint"
  value       = var.use_existing_cluster ? var.kubernetes_host : try(module.eks[0].cluster_endpoint, null)
}

output "cluster_ca_certificate" {
  description = "Kubernetes cluster CA certificate (base64 encoded)"
  value       = var.use_existing_cluster ? var.kubernetes_ca_cert : try(module.eks[0].cluster_ca_certificate, null)
  sensitive   = true
}

# VPC Information (if created)
output "vpc_id" {
  description = "VPC ID"
  value       = try(module.vpc[0].vpc_id, null)
}

output "private_subnet_ids" {
  description = "Private subnet IDs"
  value       = try(module.vpc[0].private_subnets, null)
}

output "public_subnet_ids" {
  description = "Public subnet IDs"
  value       = try(module.vpc[0].public_subnets, null)
}

# Fen Deployment Information
output "namespace" {
  description = "Kubernetes namespace for Fen"
  value       = local.namespace
}

output "fen_version" {
  description = "Deployed Fen version"
  value       = var.fen_version
}

output "num_shards" {
  description = "Number of data shards"
  value       = var.num_shards
}

# Kafka Information
output "kafka_bootstrap_servers" {
  description = "Kafka bootstrap servers"
  value       = var.install_kafka ? "fen-kafka-kafka-bootstrap.${local.namespace}:9092" : var.kafka_bootstrap_servers
}

# Connection Information
output "kubectl_config" {
  description = "Command to configure kubectl"
  value       = var.use_existing_cluster ? "kubectl already configured" : "aws eks update-kubeconfig --name ${local.cluster_name} --region ${var.aws_region}"
}

output "api_endpoint" {
  description = "Fen API endpoint (within cluster)"
  value       = "http://fen-api.${local.namespace}:3000"
}

output "coordinator_endpoint" {
  description = "Coordinator API endpoint (within cluster)"
  value       = var.cluster_enabled ? "http://fen-coordinator.${local.namespace}:9002" : "N/A (standalone mode)"
}
