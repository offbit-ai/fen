# Terraform configuration for Fen infrastructure deployment
#
# This configuration deploys Fen to a Kubernetes cluster (EKS, GKE, or AKS)
# with all required infrastructure components.
#
# Usage:
#   cd deploy/terraform
#   terraform init
#   terraform plan -var-file=environments/dev.tfvars
#   terraform apply -var-file=environments/dev.tfvars

terraform {
  required_version = ">= 1.5.0"

  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.25"
    }
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.12"
    }
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.30"
    }
  }

  # Uncomment for remote state storage
  # backend "s3" {
  #   bucket = "fen-terraform-state"
  #   key    = "infrastructure/terraform.tfstate"
  #   region = "us-west-2"
  # }
}

# AWS Provider configuration
provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project     = "fen"
      Environment = var.environment
      ManagedBy   = "terraform"
    }
  }
}

# Kubernetes provider - configured after EKS cluster creation
provider "kubernetes" {
  host                   = var.use_existing_cluster ? var.kubernetes_host : module.eks[0].cluster_endpoint
  cluster_ca_certificate = var.use_existing_cluster ? base64decode(var.kubernetes_ca_cert) : base64decode(module.eks[0].cluster_ca_certificate)

  exec {
    api_version = "client.authentication.k8s.io/v1beta1"
    command     = "aws"
    args        = ["eks", "get-token", "--cluster-name", var.use_existing_cluster ? var.cluster_name : module.eks[0].cluster_name]
  }
}

# Helm provider
provider "helm" {
  kubernetes {
    host                   = var.use_existing_cluster ? var.kubernetes_host : module.eks[0].cluster_endpoint
    cluster_ca_certificate = var.use_existing_cluster ? base64decode(var.kubernetes_ca_cert) : base64decode(module.eks[0].cluster_ca_certificate)

    exec {
      api_version = "client.authentication.k8s.io/v1beta1"
      command     = "aws"
      args        = ["eks", "get-token", "--cluster-name", var.use_existing_cluster ? var.cluster_name : module.eks[0].cluster_name]
    }
  }
}

# Local values
locals {
  cluster_name = var.use_existing_cluster ? var.cluster_name : module.eks[0].cluster_name
  namespace    = var.namespace

  common_labels = {
    app         = "fen"
    environment = var.environment
  }
}

# VPC Module (only if creating new infrastructure)
module "vpc" {
  count   = var.use_existing_cluster ? 0 : 1
  source  = "terraform-aws-modules/vpc/aws"
  version = "~> 5.0"

  name = "${var.cluster_name}-vpc"
  cidr = var.vpc_cidr

  azs             = var.availability_zones
  private_subnets = var.private_subnet_cidrs
  public_subnets  = var.public_subnet_cidrs

  enable_nat_gateway   = true
  single_nat_gateway   = var.environment == "dev"
  enable_dns_hostnames = true
  enable_dns_support   = true

  public_subnet_tags = {
    "kubernetes.io/role/elb" = 1
  }

  private_subnet_tags = {
    "kubernetes.io/role/internal-elb" = 1
  }

  tags = local.common_labels
}

# EKS Module (only if creating new infrastructure)
module "eks" {
  count   = var.use_existing_cluster ? 0 : 1
  source  = "terraform-aws-modules/eks/aws"
  version = "~> 20.0"

  cluster_name    = var.cluster_name
  cluster_version = var.cluster_version

  vpc_id     = module.vpc[0].vpc_id
  subnet_ids = module.vpc[0].private_subnets

  cluster_endpoint_public_access = true

  eks_managed_node_groups = {
    coordinator = {
      name           = "coordinator"
      instance_types = ["m6i.large"]
      min_size       = 3
      max_size       = 3
      desired_size   = 3

      labels = {
        "fen.io/role" = "coordinator"
      }

      taints = [{
        key    = "fen.io/dedicated"
        value  = "coordinator"
        effect = "NO_SCHEDULE"
      }]
    }

    data = {
      name           = "data"
      instance_types = ["r6i.xlarge"]
      min_size       = var.num_shards
      max_size       = var.num_shards * 3
      desired_size   = var.num_shards

      labels = {
        "fen.io/role" = "data"
      }

      taints = [{
        key    = "fen.io/dedicated"
        value  = "data"
        effect = "NO_SCHEDULE"
      }]
    }

    compute = {
      name           = "compute"
      instance_types = ["c6i.xlarge"]
      min_size       = 2
      max_size       = 20
      desired_size   = 4

      labels = {
        "fen.io/role" = "compute"
      }
    }

    gateway = {
      name           = "gateway"
      instance_types = ["m6i.large"]
      min_size       = 2
      max_size       = 10
      desired_size   = 2

      labels = {
        "fen.io/role" = "gateway"
      }
    }
  }

  tags = local.common_labels
}

# Create namespace
resource "kubernetes_namespace" "fen" {
  metadata {
    name = local.namespace

    labels = local.common_labels
  }

  depends_on = [module.eks]
}

# Install Strimzi Kafka Operator
resource "helm_release" "strimzi" {
  count = var.install_kafka ? 1 : 0

  name             = "strimzi"
  repository       = "https://strimzi.io/charts/"
  chart            = "strimzi-kafka-operator"
  namespace        = local.namespace
  version          = "0.44.0"
  create_namespace = false

  set {
    name  = "watchNamespaces"
    value = "{${local.namespace}}"
  }

  depends_on = [kubernetes_namespace.fen]
}

# Apply Kafka cluster manifest
# Note: Strimzi CRDs are applied via kubectl as they contain multiple YAML documents
# Run: kubectl apply -f ../strimzi/kafka-cluster.yaml -n fen
# Run: kubectl apply -f ../strimzi/kafka-topics.yaml -n fen
resource "null_resource" "kafka_cluster" {
  count = var.install_kafka ? 1 : 0

  provisioner "local-exec" {
    command = <<-EOT
      kubectl apply -f ${path.module}/../strimzi/kafka-cluster.yaml -n ${local.namespace}
      kubectl apply -f ${path.module}/../strimzi/kafka-topics.yaml -n ${local.namespace}
    EOT
  }

  depends_on = [helm_release.strimzi]
}

# Install Fen via Helm
resource "helm_release" "fen" {
  name             = "fen"
  chart            = "${path.module}/../helm/fen"
  namespace        = local.namespace
  create_namespace = false

  values = [
    file("${path.module}/../helm/fen/values.yaml"),
    var.helm_values_file != "" ? file(var.helm_values_file) : ""
  ]

  set {
    name  = "image.tag"
    value = var.fen_version
  }

  set {
    name  = "cluster.shards"
    value = var.num_shards
  }

  set {
    name  = "cluster.enabled"
    value = var.cluster_enabled
  }

  set {
    name  = "kafka.strimzi.enabled"
    value = var.install_kafka
  }

  depends_on = [
    kubernetes_namespace.fen,
    helm_release.strimzi
  ]
}

# EBS CSI Driver (for persistent volumes)
resource "helm_release" "ebs_csi" {
  count = var.use_existing_cluster ? 0 : 1

  name       = "aws-ebs-csi-driver"
  repository = "https://kubernetes-sigs.github.io/aws-ebs-csi-driver"
  chart      = "aws-ebs-csi-driver"
  namespace  = "kube-system"
  version    = "2.25.0"

  set {
    name  = "controller.serviceAccount.create"
    value = "true"
  }

  depends_on = [module.eks]
}
