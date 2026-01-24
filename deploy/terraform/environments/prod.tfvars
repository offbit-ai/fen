# Production environment configuration for Fen
#
# Usage:
#   terraform plan -var-file=environments/prod.tfvars
#   terraform apply -var-file=environments/prod.tfvars

# Environment
environment = "prod"
namespace   = "fen"

# Cluster configuration
cluster_name    = "fen-cluster-prod"
cluster_version = "1.29"

# Use existing cluster (set to true if deploying to existing K8s)
use_existing_cluster = false

# Fen configuration
fen_version     = "v0.1.0" # Use specific version tag in production
num_shards      = 16
cluster_enabled = true
install_kafka   = true

# AWS region
aws_region = "us-west-2"

# VPC configuration (production sized)
vpc_cidr             = "10.0.0.0/16"
availability_zones   = ["us-west-2a", "us-west-2b", "us-west-2c"]
private_subnet_cidrs = ["10.0.1.0/24", "10.0.2.0/24", "10.0.3.0/24"]
public_subnet_cidrs  = ["10.0.101.0/24", "10.0.102.0/24", "10.0.103.0/24"]

# Instance types (production sized)
coordinator_instance_type = "m6i.large"
data_instance_type        = "r6i.xlarge"
compute_instance_type     = "c6i.xlarge"
gateway_instance_type     = "m6i.large"
