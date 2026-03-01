---
name: indices-run-operations
description: Use this skill when finding tasks to execute and creating/listing/inspecting runs with the Indices CLI.
---

# Indices Run Operations

## When To Use

Use this skill for:
- Finding task IDs before execution
- `indices runs create`
- `indices runs list`
- `indices runs get`

## Find A Task To Run

Use tasks commands to locate task IDs:

```bash
indices tasks list --output json
indices tasks get <task-uuid>
```

## Create Run

### Argument mode (default)

```bash
indices runs create \
  --task-id "<task-uuid>" \
  --arguments '{"job_id":"A123"}' \
  --secret-bindings '{"login":"<secret-uuid>"}'
```

Argument-mode flags:
- `--task-id <uuid>` (required)
- `--arguments <json-object>`
- `--secret-bindings <json-object>`

### Explicit JSON source

```bash
indices runs create --body '{"task_id":"<task-uuid>","arguments":{"job_id":"A123"},"secret_bindings":{}}'
indices runs create --file ./run.json
cat run.json | indices runs create
```

Rules:
- Use at most one of `--body`, `--file`, `--stdin`.
- Do not mix explicit JSON source flags with argument-mode flags.
- If no args/source are provided and stdin is piped, JSON is read from stdin.

## List Runs

```bash
indices runs list --task-id <task-uuid>
indices runs list --task-id <task-uuid> --limit 20
```

Notes:
- `--task-id` is required.
- `--cursor` is currently unsupported and returns an error.

## Get Run

```bash
indices runs get <run-uuid>
```
