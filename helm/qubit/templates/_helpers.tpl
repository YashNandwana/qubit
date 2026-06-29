{{/*
Expand the name of the chart.
*/}}
{{- define "qubit.name" -}}
{{- .Chart.Name }}
{{- end }}

{{/*
Full release name, capped at 63 chars.
*/}}
{{- define "qubit.fullname" -}}
{{- if contains .Chart.Name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name .Chart.Name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}

{{- define "qubit.core.fullname" -}}
{{- printf "%s-core" (include "qubit.fullname" .) }}
{{- end }}

{{- define "qubit.clusterAgent.fullname" -}}
{{- printf "%s-cluster-agent" (include "qubit.fullname" .) }}
{{- end }}

{{- define "qubit.ebpfLoader.fullname" -}}
{{- printf "%s-ebpf-loader" (include "qubit.fullname" .) }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "qubit.labels" -}}
helm.sh/chart: {{ .Chart.Name }}-{{ .Chart.Version }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Core gRPC address used by cluster-agent and ebpf-loader.
*/}}
{{- define "qubit.core.grpcAddress" -}}
{{- printf "%s.%s.svc.cluster.local:%d" (include "qubit.core.fullname" .) .Release.Namespace (.Values.core.grpcPort | int) }}
{{- end }}

{{/*
ClickHouse host.
*/}}
{{- define "qubit.clickhouse.host" -}}
{{- required "clickhouse.host is required" .Values.clickhouse.host }}
{{- end }}
