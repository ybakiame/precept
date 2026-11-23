---
name: precept-connect
description: Connect this coding agent to Precept, the developer's local store of reviewed rules, project memory and work sessions. Use when the developer asks to set up Precept in a harness other than Claude Code or Codex (Qoder, CodeBuddy, Cursor, Antigravity, Gemini CLI and others), or when Precept's MCP tools or briefings are missing in this harness.
---

# Connect this agent to Precept

Precept is a local daemon on the developer's machine. It serves the developer's
confirmed rules, the repository's rules in `.precept/rules/`, project memory and
work sessions to coding agents. Precept itself sets up Claude Code and Codex from
its desktop app; every other harness connects itself by following this skill.
That connection is the developer's own configuration, not a supported Precept
integration: Precept's desktop app will not show it, and nothing here changes
Precept.

## Rules for you, the agent

- Ask the developer before editing any configuration file of this harness, show
  the exact change, and keep a copy of the file you change.
- Use only the values in **This installation** below. Never invent paths, never
  read or copy Precept's credential files, never pass an operator or write
  credential, and never edit anything under Precept's state or runtime
  directories.
- Change only entries named `precept` (MCP) or commands that run `precept-hook`
  (hooks). Leave every other entry as it is.
- If this harness's hook format is not described below, do not adapt one by
  guessing: connect MCP only (step 1) and stop there.

## Step 1: the MCP server (every harness that speaks MCP)

Add one stdio MCP server named `precept` with the command and arguments from
**This installation**. Most harnesses accept this JSON shape in their MCP
settings:

```json
{"mcpServers": {"precept": {"command": "<precept-mcp>", "args": ["--runtime-dir", "<dir>", "--endpoint", "<endpoint>"]}}}
```

One server serves every project the developer registered in Precept. It binds to
the folder the session runs in (or the workspace roots the harness reports) and
serves nothing outside a registered project. Its tools:

- `get_effective_context`: the rules that apply here. Call it before planning or
  editing, and again after the conversation was compacted.
- `explain_rule`: why a rule applies or not.
- `get_handoff`: resume saved work by task id.
- Where the developer allowed agent writes: `suggest_rule`, the work-session
  tools (`start_session`, `update_work_state`, `close_session`) and project
  memory (`remember`, `recall`, `reaffirm`, `forget`).

Restart the session after adding the server, then call `get_effective_context`.
Its answer names the project; an error saying the folder is not a registered
project means the developer has to register it in Precept first.

## Step 2: hooks (only for harnesses that read Claude Code's hook format)

Some harnesses (for example Qoder and CodeBuddy) document hooks in exactly Claude
Code's format: a `hooks` object in a settings file, events such as
`SessionStart`, `PreToolUse`, `PostToolUse` and `PreCompact`, the hook input as
JSON on stdin with `cwd`, `tool_name` and `tool_input`, and JSON on stdout with
`hookSpecificOutput`. Only if this harness's documentation says so, add the
commands from **This installation** for the project:

| Event | Matcher | What the agent gets |
| --- | --- | --- |
| `SessionStart` | none | A briefing of the rules, open work, recently blocked rules and project memory at every start and after compaction |
| `PreToolUse` | `Bash\|Write\|Edit\|MultiEdit\|NotebookEdit` (this harness's names for shell and file-writing tools) | The developer's opt-in gate: a call that breaks a committed hard rule is denied with the rule's reason |
| `PostToolUse` | `Read` (its file-read tool) | A note when committed rules' claims about the file just read no longer hold |
| `PreCompact` | none | A reminder to record the work session before compaction |

Each command is written in a shape like this:

```json
{"hooks": {"SessionStart": [{"hooks": [{"type": "command", "command": "<session-start command>", "timeout": 10}]}],
           "PreToolUse": [{"matcher": "Bash|Write|Edit|MultiEdit|NotebookEdit", "hooks": [{"type": "command", "command": "<pre-tool command>", "timeout": 10}]}]}}
```

Replace `<harness>` in each command with this harness's short name (lowercase,
for example `qoder`): Precept's activity log then names it. Every hook command
is read-only, stays silent outside the project's folder and fails open: if
Precept cannot answer, the session and the tool call go on unchanged. Hooks are
cooperative guidance, not a security boundary.

## Step 3: check

- A new session in the project folder starts with a "Precept: context for
  <project>" briefing (with hooks) and `get_effective_context` answers (with
  MCP).
- The developer can see the calls in Precept's desktop app, on the Sessions page
  of the project, labelled `<harness> hook` or by the MCP client's own name.

## Undo

Remove the `precept` MCP entry and every hook command that runs `precept-hook`
from this harness's configuration, or restore the copy you kept.

## This installation

Not filled in: this copy of the skill has no values for this machine. Ask the
developer to run `precept --runtime-dir <dir> --endpoint <endpoint> skill`
(add `--out <this harness's skills folder>/precept-connect` to save it there);
that prints this skill with the exact MCP command and each registered project's
hook commands. Do not guess them.
