# Development environment configuration for Fen
#
# Usage:
#   terraform plan -var-file=environments/dev.tfvars
#   terraform apply -var-file=environments/dev.tfvars

# Environment
environment = "dev"
namespace   = "fen-dev"

# Cluster configuration
cluster_name    = "fen-cluster-dev"
cluster_version = "1.29"

# Use existing cluster (set to true if deploying to existing K8s)
use_existing_cluster = false

# Fen configuration
fen_version     = "latest"
num_shards      = 2
cluster_enabled = true
install_kafka   = true

# AWS region
aws_region = "us-west-2"

# VPC configuration (smaller for dev)
vpc_cidr             = "10.0.0.0/16"
availability_zones   = ["us-west-2a", "us-west-2b"]
private_subnet_cidrs = ["10.0.1.0/24", "10.0.2.0/24"]
public_subnet_cidrs  = ["10.0.101.0/24", "10.0.102.0/24"]

# Instance types (smaller for dev)
coordinator_instance_type = "t3.medium"
data_instance_type        = "t3.large"
compute_instance_type     = "t3.large"
gateway_instance_type     = "t3.medium"
