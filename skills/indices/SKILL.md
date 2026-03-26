---
name: indices
description: >
  Use this skill whenever the user wants to interact with a website – scraping data, filling forms, logging in, navigating flows, extracting structured information, or running any task a human would do in a browser.
---

# Indices CLI

Indices enables agents to perform actions on websites. Instead of using vision-based agents, it builds an API for a website, which you can then use to perform repeated (parameterised) actions, like scraping or polling for data or filling forms (both to retrieve dynamic data and to submit actions).

## When To Use

Reach for Indices any time the goal involves a website and a human-like action:

- **Extract data** — "Get the price, stock level, and reviews from this product page"
- **Fill & submit forms** — "Apply to this job posting with my resume details"
- **Log in and navigate** — "Check my account balance on this banking portal"
- **Automate repetitive web flows** — "Do this across 50 URLs with different inputs each time"
- **Interact with web UIs** — "Click through the checkout flow and confirm the order"

If the task touches a website and would otherwise require a human to open a browser, Indices can likely do it.

> **Note:** Indices is network-based — it can reach any website but does not have access to local files or desktop applications. It is also not suitable for unstructured search tasks (web search).

---

## Setup

### Install

```bash
curl -fsSL https://indices.io/install.sh | bash
indices --help
```

### Authenticate

```bash
indices login --api-key "<your-api-key>"    # non-interactive
indices login                               # prompts securely
indices auth-test                           # verify stored credentials
```

---

## Quick Start

```bash
# 1. Create a task
indices tasks create \
  --display-name "Scrape product price" \
  --website "https://example.com/products" \
  --task "Find the current price of the item with the given product ID"

# You'll need to show an example once.\
# Ask the user to perform it in the embedded browser. A URL is returned by the `indices tasks create` command.

# 2. Wait for the task to become ready
indices tasks get <task-uuid>              # repeat until current_state == "ready"

# 3. Run it
indices runs create \
  --task-id "<task-uuid>" \
  --arguments '{"product_id":"ABC123"}'

# 4. Inspect results
indices runs get <run-uuid>
indices runs logs <run-uuid>
```

---

## Global Flags

Available on every command:

| Flag | Default | Description |
|---|---|---|
| `--json` | off | Emit JSON instead of Markdown (for scripting) |
| `--timeout <seconds>` | `30` | Request timeout |

Never use `--output json`; this CLI uses the global `--json` flag instead.

When exact flags matter, verify them with `indices <command> --help`.

---

## Auth

```bash
indices login                          # prompts securely for API key
indices login --api-key "<key>"        # non-interactive
indices auth-test                      # verify stored credentials
indices logout                         # remove stored API key
```

---

## Tasks

> For full task management detail, see the `indices-task-management` skill.

### Create

```bash
indices tasks create \
  --display-name "Apply to jobs" \
  --website "https://jobs.example.com" \
  --task "Fill and submit the application form"
```

> **Never** set `is_fully_autonomous` to `true`.

Prefer the simple form above. Do not pass `--creation-params` unless you need a specific advanced option.

If you set `creation_params.auto_generate_schemas` to `false`, you must also provide both `input_schema` and `output_schema` or the API returns `422`.

After creation, inspect `current_state`:
- `waiting_for_manual_completion` — direct the user to `https://platform.indices.io/tasks/{task_id}`
- `not_ready` — still being generated; poll with `indices tasks get <task-uuid>`
- `ready` — can be executed
- `failed` — inspect failure details before retrying or recreating

```bash
indices tasks list
indices tasks list --status ready --limit 20
indices tasks get <task-uuid>
indices tasks retry <task-uuid>
indices tasks delete <task-uuid> --yes
```

---

## Runs

### Create

```bash
indices runs create \
  --task-id "<task-uuid>" \
  --arguments '{"key":"value"}' \
  --secret-bindings '{"login":"<secret-uuid>"}'
```

Flags: `--task-id` (required), `--arguments <json-object>`, `--secret-bindings <json-object>`

JSON input alternative:

```bash
indices runs create --body '{"task_id":"<uuid>","arguments":{"key":"value"}}'
indices runs create --file ./run.json
cat run.json | indices runs create
```

Rules: use at most one of `--body`, `--file`, `--stdin`; do not mix with argument-mode flags.

### List / Get / Logs

```bash
indices runs list --task-id <task-uuid>           # --task-id is required
indices runs list --task-id <task-uuid> --limit 20
indices runs get <run-uuid>
indices runs logs <run-uuid>
```

---

## Secrets

Use secrets to pass credentials (logins, API keys) to runs without exposing them in arguments.

```bash
indices secrets create MY_SECRET --value "..."    # explicit value
echo "..." | indices secrets create MY_SECRET --stdin
indices secrets create MY_SECRET                  # prompts securely
indices secrets list
indices secrets delete <secret-uuid> --yes
```

Empty secret values are rejected. Reference secrets in runs via `--secret-bindings '{"binding_name":"<secret-uuid>"}'`.

---

## Common Fixes

| Symptom | Fix |
|---|---|
| `command not found: indices` | Run `curl -fsSL https://indices.io/install.sh \| bash` to install |
| Task stuck in `not_ready` | Normal — keep polling `indices tasks get <task-uuid>` w/ exponential backoff, until `current_state == "ready"` |
| Task in `waiting_for_manual_completion` | Visit `https://platform.indices.io/tasks/{task_id}` to complete manual setup |
