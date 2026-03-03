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
```

> **Never** set `is_fully_autonomous` to `true` (in any argument mode)

Argument-mode flags:
- `--display-name <string>` (required)
- `--website <url>` (required)
- `--task <string>` (required)
- `--input-schema <string>`
- `--output-schema <string>`
- `--creation-params <json-object>`

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

Tasks created without autonomous mode require manual completion in a browser. Direct the user to `https://platform.indices.io/tasks/{task_id}` (substituting the actual task ID) to complete it.

## List Tasks

```bash
indices tasks list
indices tasks list --status ready --limit 20
```

Notes:
- Valid `--status`: `not_ready`, `waiting_for_manual_completion`, `ready`, `failed`
- `--cursor` is currently unsupported and returns an error

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
