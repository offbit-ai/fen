{{/*
Expand the name of the chart.
*/}}
{{- define "fen.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "fen.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "fen.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "fen.labels" -}}
helm.sh/chart: {{ include "fen.chart" . }}
{{ include "fen.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "fen.selectorLabels" -}}
app.kubernetes.io/name: {{ include "fen.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "fen.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "fen.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Get the image name for a component
*/}}
{{- define "fen.image" -}}
{{- $registry := .global.imageRegistry | default .image.registry -}}
{{- $repository := .image.repository | default .global.repository -}}
{{- $tag := .image.tag | default .global.tag | default "latest" -}}
{{- if $registry -}}
{{- printf "%s/%s/%s:%s" $registry $repository .component $tag -}}
{{- else -}}
{{- printf "%s/%s:%s" $repository .component $tag -}}
{{- end -}}
{{- end }}

{{/*
Coordinator peer addresses
*/}}
{{- define "fen.coordinatorPeers" -}}
{{- $fullname := include "fen.fullname" . -}}
{{- $replicas := int .Values.coordinator.replicas -}}
{{- $peers := list -}}
{{- range $i := until $replicas -}}
{{- $peers = append $peers (printf "%s-coordinator-%d.%s-coordinator-headless:%d" $fullname $i $fullname (int $.Values.coordinator.service.raftPort)) -}}
{{- end -}}
{{- join "," $peers -}}
{{- end }}

{{/*
Kafka bootstrap servers
*/}}
{{- define "fen.kafkaBootstrap" -}}
{{- if .Values.kafka.bootstrapServers -}}
{{- .Values.kafka.bootstrapServers -}}
{{- else -}}
{{- printf "%s-kafka-bootstrap:9092" .Values.kafka.cluster.name -}}
{{- end -}}
{{- end }}

{{/*
Data node headless service name for a shard
*/}}
{{- define "fen.dataHeadlessService" -}}
{{- $fullname := include "fen.fullname" . -}}
{{- printf "%s-data-shard-%d-headless" $fullname .shardIndex -}}
{{- end }}
