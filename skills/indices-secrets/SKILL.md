---
name: indices-secrets
description: Use this skill when managing secret values used by Indices runs via the CLI.
---

# Indices Secrets

## When To Use

Use this skill for:
- `indices secrets create`
- `indices secrets list`
- `indices secrets delete`

## Create Secret

```bash
indices secrets create OPENAI_API_KEY --value "sk-..."
echo "sk-..." | indices secrets create OPENAI_API_KEY --stdin
indices secrets create OPENAI_API_KEY
```

Behavior:
- Uses `--value` if provided.
- Reads stdin when `--stdin` is provided.
- Otherwise prompts securely for the value.
- Empty secret values are rejected.

## List Secrets

```bash
indices secrets list
```

## Delete Secret

```bash
indices secrets delete <secret-uuid>
indices secrets delete <secret-uuid> --yes
```

Without `--yes`, CLI asks for confirmation.
