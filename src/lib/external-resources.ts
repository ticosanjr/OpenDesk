export type ExternalResourceGroup = "skills" | "mcp" | "workflows";

export interface ExternalResource {
  id: string;
  group: ExternalResourceGroup;
  name: string;
  url: string;
  focus: string;
  usage: string;
  tags: string[];
  /** Template que já cria a estrutura mais adequada para usar a fonte. */
  templateId?: string;
}

export const RESOURCE_GROUPS: Array<{
  id: ExternalResourceGroup | "all";
  label: string;
  description: string;
}> = [
  {
    id: "all",
    label: "Todos",
    description: "Skills, MCPs, workflows e prompts.",
  },
  {
    id: "skills",
    label: "Skills & agentes",
    description: "Instruções, personas, slash commands e capacidades reutilizáveis.",
  },
  {
    id: "mcp",
    label: "MCP",
    description: "Ferramentas externas, bancos, cloud e automação via Model Context Protocol.",
  },
  {
    id: "workflows",
    label: "Workflows & prompts",
    description: "Templates multi-step, RAG, webhooks e engenharia de prompt.",
  },
];

export const EXTERNAL_RESOURCES: ExternalResource[] = [
  {
    id: "skills-sh",
    group: "skills",
    name: "skills.sh",
    url: "https://skills.sh",
    focus: "Hub aberto de skills padronizadas em SKILL.md.",
    usage: "Encontre uma skill, copie a pasta e importe no Studio.",
    tags: ["SKILL.md", "skills", "agentes"],
    templateId: "fabrica-capacidades",
  },
  {
    id: "officialskills",
    group: "skills",
    name: "officialskills.sh",
    url: "https://officialskills.sh",
    focus: "Diretório curado de skills mantidas por times de engenharia.",
    usage: "Use como referência de qualidade antes de adaptar uma skill.",
    tags: ["curadoria", "oficial", "SKILL.md"],
    templateId: "fabrica-capacidades",
  },
  {
    id: "linkly",
    group: "skills",
    name: "Linkly AI Skills Hub",
    url: "https://linkly.ai/skills",
    focus: "Ranking diário por instalações e métricas de skills populares.",
    usage: "Compare adoção e procure skills com uso real antes de instalar.",
    tags: ["ranking", "métricas", "skills"],
    templateId: "fabrica-capacidades",
  },
  {
    id: "clawhub",
    group: "skills",
    name: "ClawHub",
    url: "https://hub.openclaw.ai",
    focus: "Automação de terminal, sistema local e comandos de shell.",
    usage: "Ótimo para skills operacionais; revise comandos destrutivos antes de habilitar.",
    tags: ["shell", "terminal", "automação"],
    templateId: "fabrica-capacidades",
  },
  {
    id: "github-agent-skills",
    group: "skills",
    name: "GitHub Topics · agent-skills",
    url: "https://github.com/topics/agent-skills",
    focus: "Repositórios, awesome-lists e implementações abertas de skills.",
    usage: "Clone o repositório e importe a pasta da skill pelo Studio.",
    tags: ["GitHub", "open source", "awesome-list"],
    templateId: "fabrica-capacidades",
  },
  {
    id: "flowgpt",
    group: "skills",
    name: "FlowGPT",
    url: "https://flowgpt.com",
    focus: "Personas, system instructions e prompts comunitários.",
    usage: "Adapte o prompt ao seu agente; não cole instruções sem revisar.",
    tags: ["prompts", "personas", "instruções"],
    templateId: "conteudo-midia",
  },
  {
    id: "promptbase",
    group: "skills",
    name: "PromptBase",
    url: "https://promptbase.com",
    focus: "Prompts especializados e comandos reutilizáveis.",
    usage: "Use como inspiração para comandos e system prompts.",
    tags: ["prompts", "comandos"],
    templateId: "conteudo-midia",
  },
  {
    id: "smithery",
    group: "mcp",
    name: "Smithery",
    url: "https://smithery.ai",
    focus: "Maior registro indexado de servidores MCP comunitários e verificados.",
    usage: "Instale com a CLI do Smithery ou copie o comando para o opencode.jsonc.",
    tags: ["MCP", "CLI", "registry"],
    templateId: "fabrica-capacidades",
  },
  {
    id: "glama",
    group: "mcp",
    name: "Glama · MCP Explorer",
    url: "https://glama.ai/mcp/servers",
    focus: "Diretório visual com status, tools, resources e schemas.",
    usage: "Copie a configuração JSON para o bloco de MCPs do workspace.",
    tags: ["MCP", "schemas", "tools"],
    templateId: "fabrica-capacidades",
  },
  {
    id: "anthropic-mcp",
    group: "mcp",
    name: "Anthropic MCP Servers",
    url: "https://github.com/modelcontextprotocol/servers",
    focus: "Referência oficial: Filesystem, Postgres, Git, Fetch e SQLite.",
    usage: "Use `npx @modelcontextprotocol/server-<nome>` e configure no workspace.",
    tags: ["oficial", "MCP", "npx"],
    templateId: "fabrica-capacidades",
  },
  {
    id: "pulsemcp",
    group: "mcp",
    name: "PulseMCP",
    url: "https://pulsemcp.com",
    focus: "Lançamentos, tendências e novidades diárias do ecossistema MCP.",
    usage: "Descubra servidores novos e abra o repositório para ver o setup.",
    tags: ["MCP", "novidades", "trends"],
    templateId: "fabrica-capacidades",
  },
  {
    id: "awesome-mcp",
    group: "mcp",
    name: "Awesome MCP Servers",
    url: "https://github.com/punkpeye/awesome-mcp-servers",
    focus: "Lista comunitária categorizada por banco, cloud e devtools.",
    usage: "Use como mapa de descoberta antes de escolher um servidor.",
    tags: ["MCP", "awesome-list", "GitHub"],
    templateId: "fabrica-capacidades",
  },
  {
    id: "mcp-so",
    group: "mcp",
    name: "mcp.so",
    url: "https://mcp.so",
    focus: "Catálogo com busca por tags, casos de uso e linguagem.",
    usage: "Copie snippets prontos para o bloco `mcp` do opencode.jsonc.",
    tags: ["MCP", "busca", "snippets"],
    templateId: "fabrica-capacidades",
  },
  {
    id: "composio",
    group: "mcp",
    name: "Composio Toolkits / MCP",
    url: "https://composio.dev/mcp",
    focus: "250+ integrações com autenticação OAuth gerenciada.",
    usage: "Use `npx composio-mcp` e conecte Linear, GitHub, Slack e outros.",
    tags: ["MCP", "OAuth", "integrações"],
    templateId: "gtm-clientes",
  },
  {
    id: "docker-mcp",
    group: "mcp",
    name: "Docker MCP Catalog",
    url: "https://hub.docker.com/search?q=mcp-server",
    focus: "Imagens isoladas para executar MCPs sem poluir o runtime local.",
    usage: "Execute com `docker run -i --rm <imagem>` e registre no workspace.",
    tags: ["MCP", "Docker", "isolamento"],
    templateId: "fabrica-capacidades",
  },
  {
    id: "cursor-directory",
    group: "mcp",
    name: "Cursor Directory · MCP",
    url: "https://cursor.directory/mcp",
    focus: "Servidores e contextos voltados a editores e IDEs.",
    usage: "Use como referência de configuração para ferramentas de código.",
    tags: ["MCP", "IDE", "devtools"],
    templateId: "fabrica-capacidades",
  },
  {
    id: "dify",
    group: "workflows",
    name: "Dify Marketplace",
    url: "https://marketplace.dify.ai",
    focus: "Templates visuais de agentes, fluxos RAG e pacotes .yml.",
    usage: "Use como blueprint conceitual para receitas e fluxos multi-step.",
    tags: ["workflows", "RAG", "templates"],
    templateId: "operacoes-backoffice",
  },
  {
    id: "n8n",
    group: "workflows",
    name: "n8n Templates",
    url: "https://n8n.io/workflows",
    focus: "Blueprints JSON de workflows multi-step, subagentes e webhooks.",
    usage: "Adapte o desenho para receitas com gatilhos, HITL e sinks.",
    tags: ["workflows", "webhooks", "JSON"],
    templateId: "operacoes-backoffice",
  },
];
