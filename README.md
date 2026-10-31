# downloadsort

## Working with OpenCode

OpenCode works in this repo out of the box:

- `AGENTS.md` is loaded automatically as agent instructions.
- Skills in `.agents/skills/` are discovered automatically (OpenCode's project compatibility path), so no extra config is needed.
- `opencode.json` holds project-level OpenCode settings. It stays free of secrets.

The Super Productivity MCP server is configured in the global OpenCode config (`~/.config/opencode/opencode.json`), so it is available in every project and its access key stays out of this public repo.

To add a project-scoped MCP server instead, put it under `mcp.servers` in `opencode.json` and read the secret from the environment rather than committing it:

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "servers": {
      "my-server": {
        "type": "remote",
        "url": "https://mcp.example.com/mcp",
        "oauth": false,
        "headers": { "Authorization": "Bearer {env:MY_SERVER_TOKEN}" }
      }
    }
  }
}
```
