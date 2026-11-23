use super::*;

pub async fn catalog() -> Json<Value> {
    Json(json!({
        "repos": [
            {
                "id": "anthropic-skills",
                "name": "Anthropic Skills",
                "description": "Coleção oficial de skills de exemplo da Anthropic.",
                "source_type": "github",
                "source": "anthropics/skills",
                "kind": "skill"
            },
            {
                "id": "superpowers",
                "name": "Superpowers",
                "description": "Workflows e skills de engenharia para agentes.",
                "source_type": "github",
                "source": "obra/superpowers",
                "kind": "skill"
            },
            {
                "id": "playwright-mcp",
                "name": "Playwright MCP",
                "description": "Automação de navegador via MCP (Microsoft).",
                "source_type": "npm",
                "source": "@playwright/mcp",
                "kind": "mcp"
            },
            {
                "id": "context7",
                "name": "Context7 MCP",
                "description": "Documentação atualizada de bibliotecas via MCP.",
                "source_type": "npm",
                "source": "@upstash/context7-mcp",
                "kind": "mcp"
            },
            {
                "id": "mcp-servers",
                "name": "MCP Servers",
                "description": "Servidores MCP de referência da comunidade.",
                "source_type": "github",
                "source": "modelcontextprotocol/servers",
                "kind": "mcp"
            }
        ],
        "mcps": [
            {
                "id": "filesystem",
                "name": "Filesystem",
                "description": "Leitura e escrita de arquivos em um diretório permitido.",
                "server": {
                    "type": "local",
                    "command": ["npx", "-y", "@modelcontextprotocol/server-filesystem", "."],
                    "disabled": false
                }
            },
            {
                "id": "git",
                "name": "Git",
                "description": "Operações de git em repositórios locais.",
                "server": {
                    "type": "local",
                    "command": ["npx", "-y", "@modelcontextprotocol/server-git"],
                    "disabled": false
                }
            },
            {
                "id": "fetch",
                "name": "Fetch",
                "description": "Busca páginas HTTP e converte para markdown.",
                "server": {
                    "type": "local",
                    "command": ["npx", "-y", "@modelcontextprotocol/server-fetch"],
                    "disabled": false
                }
            },
            {
                "id": "memory",
                "name": "Memory",
                "description": "Memória persistente em grafo para agentes.",
                "server": {
                    "type": "local",
                    "command": ["npx", "-y", "@modelcontextprotocol/server-memory"],
                    "disabled": false
                }
            },
            {
                "id": "sequential-thinking",
                "name": "Sequential Thinking",
                "description": "Raciocínio passo a passo estruturado.",
                "server": {
                    "type": "local",
                    "command": ["npx", "-y", "@modelcontextprotocol/server-sequential-thinking"],
                    "disabled": false
                }
            },
            {
                "id": "playwright",
                "name": "Playwright",
                "description": "Automação de navegador via Playwright MCP.",
                "server": {
                    "type": "local",
                    "command": ["npx", "-y", "@playwright/mcp@latest"],
                    "disabled": false
                }
            },
            {
                "id": "browse",
                "name": "OpenCode Browse",
                "description": "Navegador real do usuário via MCP, com logins, abas, screenshots e handoff para 2FA.",
                "server": {
                    "type": "local",
                    "command": ["npx", "-y", "@opencode/browse", "mcp"],
                    "disabled": false
                }
            }
        ]
    }))
}
