---
name: indices-task-management
description: Use this skill when creating, listing, inspecting, retrying, regenerating, or deleting tasks with the Indices CLI.
---

# Indices Task Management

## When To Use

Use this skill for:
- `indices tasks create`
- `indices tasks list`
- `indices tasks get`
- `indices tasks retry`
- `indices tasks regenerate-api`
- `indices tasks delete`

## Create Task

### Argument mode (default)

```bash
indices tasks create \
  --display-name "Apply to jobs" \
  --website "https://jobs.example.com" \
  --task "Fill and submit the form"

indices --json tasks create \
  --display-name "Apply to jobs" \
  --website "https://jobs.example.com" \
  --task "Fill and submit the form"
```

> **Never** set `is_fully_autonomous` to `true` (in any argument mode)

Argument-mode flags:
- `--display-name <string>` (required)
- `--website <url>` (required)
- `--task <string>` (required)
- `--input-schema <string>`
- `--output-schema <string>`
- `--creation-params <json-object>`

Task-creation rules:
- Prefer the simple form above. Do not pass `--creation-params` unless you need a specific advanced option.
- Schema auto-generation is the default. Leave it enabled unless you are intentionally providing manual schemas.
- If you disable schema auto-generation with `{"auto_generate_schemas":false}`, you must also provide both `input_schema` and `output_schema` or the API returns `422`.
- If you provide manual schemas, provide both `--input-schema` and `--output-schema`.

### Explicit JSON source

```bash
indices tasks create --body '{"display_name":"Apply","website":"https://jobs.example.com","task":"Submit form","creation_params":{}}'
indices tasks create --file ./task.json
cat task.json | indices tasks create
```

Rules:
- Use at most one of `--body`, `--file`, `--stdin`.
- Do not mix explicit JSON source flags with argument-mode flags.
- If no args/source are provided and stdin is piped, JSON is read from stdin.

### After creating a task

Inspect the returned `current_state` before deciding the next step:
- If `current_state` is `waiting_for_manual_completion`, direct the user to `https://platform.indices.io/tasks/{task_id}`.
- If `current_state` is `not_ready`, the task is still being generated; poll with `indices tasks get <task-uuid>`.
- If `current_state` is `ready`, it can be executed.
- If `current_state` is `failed`, inspect the failure details before retrying or recreating anything.

## List Tasks

```bash
indices tasks list
indices --json tasks list
indices tasks list --status ready --limit 20
```

Notes:
- Valid `--status`: `not_ready`, `waiting_for_manual_completion`, `ready`, `failed`
- `--json` is a global flag; do not use `--output json`

## Failure Handling

If `tasks list`, `tasks get`, or `tasks create` returns `failed to serialize or parse response` or reports a missing response field:
- Stop and treat it as CLI/API version drift
- Run `indices --version` and `which indices`
- If you are in the CLI repo, retry with `cargo run -- ...` or reinstall with `cargo install --path .`
- Do not create more tasks as a workaround until the mismatch is resolved

## Get, Retry, Regenerate API

```bash
indices tasks get <task-uuid>
indices tasks retry <task-uuid>
indices tasks regenerate-api <task-uuid>
```

## Delete Task

```bash
indices tasks delete <task-uuid>
indices tasks delete <task-uuid> --yes
```
