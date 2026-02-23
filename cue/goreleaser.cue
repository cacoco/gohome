package goreleaser

import "cue.dev/x/goreleaser"

files: aarch64: goreleaser.#Project & {
	version:      2
	project_name: "gohome"

	archives: [{
		id: "gohome"
		formats: ["tar.gz"]
		ids: ["gohome"]
		name_template: "{{ .ProjectName }}_{{- title .Os }}_{{- if eq .Arch \"amd64\" }}x86_64{{- else if eq .Arch \"386\" }}i386{{- else if .Arm }}v{{ .Arm }}{{- else }}{{ .Arch }}{{ end }}"
	}]

	before: hooks: [
		"cargo fetch --locked",
	]

	builds: [{
		id:      "gohome"
		builder: "rust"
		binary:  "gohome"
		flags: ["--release"]
		targets: ["aarch64-unknown-linux-musl"]
		mod_timestamp: "{{ .CommitTimestamp }}"
	}]

	changelog: {
		sort: "asc"
		filters: exclude: ["^docs:", "^test:"]
	}

	checksum: name_template: "{{ .ProjectName }}_aarch64_checksums.txt"

	dockers_v2: [{
		id: "gohome"
		ids: ["gohome"]
		images: ["ghcr.io/cacoco/gohome"]
		tags: ["{{ .Tag }}", "latest"]
		dockerfile: "aarch64/Dockerfile"
		extra_files: [
			"entrypoint.sh",
			"static/base.css",
			"templates/all.hbs",
			"templates/base.hbs",
			"templates/delete.hbs",
			"templates/detail.hbs",
			"templates/help.hbs",
			"templates/home.hbs",
			"templates/success.hbs",
		]
		labels: {
			"org.opencontainers.image.create":      "{{.Date}}"
			"org.opencontainers.image.title":       "{{.ProjectName}}"
			"org.opencontainers.image.revision":    "{{.FullCommit}}"
			"org.opencontainers.image.version":     "{{.Version}}"
			"org.opencontainers.image.description": "A Rust-powered port of the Tailscale [golink](https://github.com/tailscale/golink)"
		}
		platforms: ["linux/arm64"]
		flags: ["--provenance=false"]
		sbom: false
	}]

	docker_signs: [{
		id: "gohome"
		ids: ["gohome"]
		cmd:       "cosign"
		artifacts: "manifests"
		args: [
			"sign",
			"${artifact}",
			"--yes",
		]
	}]

	milestones: [{
		repo: {
			owner: "cacoco"
			name:  "gohome"
		}
		close:         true
		fail_on_error: false
		name_template: "{{ .Tag }}"
	}]

	release: {
		ids: ["gohome"]
		github: {
			owner: "cacoco"
			name:  "gohome"
		}
		mode:                       "replace"
		replace_existing_artifacts: false
		footer:                     "\n---\n\nReleased by [GoReleaser](https://github.com/goreleaser/goreleaser)."
	}

	sboms: [{
		id: "gohome"
		ids: ["gohome"]
		artifacts: "archive"
		args: ["--from", "dir", "../", "--output", "cyclonedx-json=$document"]
	}]

	signs: [{
		id: "gohome"
		ids: ["gohome"]
		cmd:       "cosign"
		stdin:     "{{ .Env.COSIGN_PWD }}"
		signature: "${artifact}.sigstore.json"
		args: [
			"sign-blob",
			"--key=cosign.key",
			"--bundle=${signature}",
			"${artifact}",
			"--yes",
		]
		artifacts: "checksum"
		output:    true
	}]
}

files: x86_64: goreleaser.#Project & {
	version:      2
	project_name: "gohome"

	archives: [{
		id: "gohome"
		formats: ["tar.gz"]
		ids: ["gohome"]
		name_template: "{{ .ProjectName }}_{{- title .Os }}_{{- if eq .Arch \"amd64\" }}x86_64{{- else if eq .Arch \"386\" }}i386{{- else if .Arm }}v{{ .Arm }}{{- else }}{{ .Arch }}{{ end }}"
	}]

	before: hooks: [
		"cargo fetch --locked",
	]

	builds: [{
		id:      "gohome"
		builder: "rust"
		binary:  "gohome"
		flags: ["--release"]
		targets: ["x86_64-unknown-linux-musl"]
		mod_timestamp: "{{ .CommitTimestamp }}"
	}]

	checksum: name_template: "{{ .ProjectName }}_x86_64_checksums.txt"

	dockers_v2: [{
		id: "gohome"
		ids: ["gohome"]
		images: ["ghcr.io/cacoco/gohome"]
		tags: ["{{ .Tag }}", "latest"]
		dockerfile: "x86_64/Dockerfile"
		extra_files: [
			"entrypoint.sh",
			"static/base.css",
			"templates/all.hbs",
			"templates/base.hbs",
			"templates/delete.hbs",
			"templates/detail.hbs",
			"templates/help.hbs",
			"templates/home.hbs",
			"templates/success.hbs",
		]
		labels: {
			"org.opencontainers.image.create":      "{{.Date}}"
			"org.opencontainers.image.title":       "{{.ProjectName}}"
			"org.opencontainers.image.revision":    "{{.FullCommit}}"
			"org.opencontainers.image.version":     "{{.Version}}"
			"org.opencontainers.image.description": "A Rust-powered port of the Tailscale [golink](https://github.com/tailscale/golink)"
		}
		platforms: ["linux/amd64"]
		flags: ["--provenance=false"]
		sbom: false
	}]

	docker_signs: [{
		id: "gohome"
		ids: ["gohome"]
		cmd:       "cosign"
		artifacts: "manifests"
		args: [
			"sign",
			"${artifact}",
			"--yes",
		]
	}]

	release: {
		ids: ["gohome"]
		github: {
			owner: "cacoco"
			name:  "gohome"
		}
		mode:                       "replace"
		replace_existing_artifacts: false
		footer:                     "\n---\n\nReleased by [GoReleaser](https://github.com/goreleaser/goreleaser)."
	}

	sboms: [{
		id: "gohome"
		ids: ["gohome"]
		artifacts: "archive"
		args: ["--from", "dir", "../", "--output", "cyclonedx-json=$document"]
	}]

	signs: [{
		id: "gohome"
		ids: ["gohome"]
		cmd:       "cosign"
		stdin:     "{{ .Env.COSIGN_PWD }}"
		signature: "${artifact}.sigstore.json"
		args: [
			"sign-blob",
			"--key=cosign.key",
			"--bundle=${signature}",
			"${artifact}",
			"--yes",
		]
		artifacts: "checksum"
		output:    true
	}]
}
