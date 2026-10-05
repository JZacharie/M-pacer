{{/* Nom du chart (surchargeable) */}}
{{- define "mpacer.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{/* Nom complet (release + chart) */}}
{{- define "mpacer.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- $name := default .Chart.Name .Values.nameOverride -}}
{{- if contains $name .Release.Name -}}
{{- .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{- define "mpacer.labels" -}}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" }}
{{ include "mpacer.selectorLabels" . }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: mpacer
{{- end -}}

{{- define "mpacer.selectorLabels" -}}
app.kubernetes.io/name: {{ include "mpacer.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{- define "mpacer.serviceAccountName" -}}
{{- if .Values.serviceAccount.create -}}
{{- default (include "mpacer.fullname" .) .Values.serviceAccount.name -}}
{{- else -}}
{{- default "default" .Values.serviceAccount.name -}}
{{- end -}}
{{- end -}}

{{/* Nom du secret contenant les identifiants */}}
{{- define "mpacer.secretName" -}}
{{- default (printf "%s-credentials" (include "mpacer.fullname" .)) .Values.auth.existingSecret -}}
{{- end -}}

{{/* Nom du secret TLS */}}
{{- define "mpacer.tlsSecretName" -}}
{{- default (printf "%s-tls" (include "mpacer.fullname" .)) .Values.ingress.tls.secretName -}}
{{- end -}}

{{/* Nom du cluster PostgreSQL CloudNativePG */}}
{{- define "mpacer.postgresName" -}}
{{- default (printf "%s-pg" (include "mpacer.fullname" .)) .Values.postgresql.name -}}
{{- end -}}

{{/* Secret applicatif genere par CloudNativePG */}}
{{- define "mpacer.postgresAppSecret" -}}
{{- if eq .Values.postgresql.mode "cnpg" -}}
{{- printf "%s-app" (include "mpacer.postgresName" .) -}}
{{- else -}}
{{- .Values.postgresql.external.existingSecret -}}
{{- end -}}
{{- end -}}

{{/* Image complete */}}
{{- define "mpacer.image" -}}
{{- printf "%s:%s" .Values.image.repository (default .Chart.AppVersion .Values.image.tag) -}}
{{- end -}}
