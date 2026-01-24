# Terraform variables for Fen deployment

# AWS Configuration
variable "aws_region" {
  description = "AWS region for deployment"
  type        = string
  default     = "us-west-2"
}

# Cluster Configuration
variable "use_existing_cluster" {
  description = "Use an existing Kubernetes cluster instead of creating EKS"
  type        = bool
  default     = false
}

variable "cluster_name" {
  description = "Name of the Kubernetes/EKS cluster"
  type        = string
  default     = "fen-cluster"
}

variable "cluster_version" {
  description = "Kubernetes version for EKS"
  type        = string
  default     = "1.29"
}

variable "kubernetes_host" {
  description = "Kubernetes API host (for existing cluster)"
  type        = string
  default     = ""
}

variable "kubernetes_ca_cert" {
  description = "Kubernetes CA certificate (base64 encoded, for existing cluster)"
  type        = string
  default     = ""
  sensitive   = true
}

# Environment
variable "environment" {
  description = "Deployment environment (dev, staging, prod)"
  type        = string
  default     = "dev"

  validation {
    condition     = contains(["dev", "staging", "prod"], var.environment)
    error_message = "Environment must be one of: dev, staging, prod"
  }
}

variable "namespace" {
  description = "Kubernetes namespace for Fen"
  type        = string
  default     = "fen"
}

# Fen Configuration
variable "fen_version" {
  description = "Fen application version/image tag"
  type        = string
  default     = "latest"
}

variable "num_shards" {
  description = "Number of data shards"
  type        = number
  default     = 4

  validation {
    condition     = var.num_shards >= 1 && var.num_shards <= 256
    error_message = "Number of shards must be between 1 and 256"
  }
}

variable "cluster_enabled" {
  description = "Enable distributed cluster mode"
  type        = bool
  default     = true
}

variable "helm_values_file" {
  description = "Path to additional Helm values file"
  type        = string
  default     = ""
}

# Kafka Configuration
variable "install_kafka" {
  description = "Install Strimzi Kafka operator and cluster"
  type        = bool
  default     = true
}

variable "kafka_bootstrap_servers" {
  description = "External Kafka bootstrap servers (if not installing Strimzi)"
  type        = string
  default     = ""
}

# VPC Configuration (for new infrastructure)
variable "vpc_cidr" {
  description = "CIDR block for VPC"
  type        = string
  default     = "10.0.0.0/16"
}

variable "availability_zones" {
  description = "Availability zones for the VPC"
  type        = list(string)
  default     = ["us-west-2a", "us-west-2b", "us-west-2c"]
}

variable "private_subnet_cidrs" {
  description = "CIDR blocks for private subnets"
  type        = list(string)
  default     = ["10.0.1.0/24", "10.0.2.0/24", "10.0.3.0/24"]
}

variable "public_subnet_cidrs" {
  description = "CIDR blocks for public subnets"
  type        = list(string)
  default     = ["10.0.101.0/24", "10.0.102.0/24", "10.0.103.0/24"]
}

# Node Configuration
variable "coordinator_instance_type" {
  description = "Instance type for coordinator nodes"
  type        = string
  default     = "m6i.large"
}

variable "data_instance_type" {
  description = "Instance type for data nodes"
  type        = string
  default     = "r6i.xlarge"
}

variable "compute_instance_type" {
  description = "Instance type for compute/worker nodes"
  type        = string
  default     = "c6i.xlarge"
}

variable "gateway_instance_type" {
  description = "Instance type for gateway/API nodes"
  type        = string
  default     = "m6i.large"
}
