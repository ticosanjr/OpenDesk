# **Especificação Complementar: Módulo de Excelência 10/10 do OpenDesk (SPEC\_DELTA.md)**

**Instrução para o Agente Desenvolvedor:** A base do OpenDesk já está implementada. Este documento contém exclusivamente os **requisitos diferenciais incrementais** (extraídos dos pontos fortes de Herder, Herdr, Pi Desktop, Hermes Workspace e Claude Code) para elevar o sistema ao nível 10/10. Implemente apenas as funcionalidades listadas abaixo, integrando-as aos módulos existentes.

### **1\. Fila Resiliente e Controle de Concorrência (Padrão *Herder*)**

> 1. **Lease Time & Heartbeat de Jobs:**  
   * Adicionar as colunas lease\_expires\_at DATETIME e last\_heartbeat\_at DATETIME na tabela execution\_queue.  
   * O worker em execução deve atualizar o heartbeat a cada 5 segundos. Se o processo do agente congelar e o lease expirar (ex.: 60s sem resposta nem eventos SSE), o daemon mata o job travado e aciona retry automático ou move para falha.  
> 2. **Grupos de Concorrência por Recurso (concurrency\_group):**  
   * Permitir classificar agentes/tarefas em grupos de limite no Rust (local\_llm\_heavy, web\_scraper, light\_api).  
   * Exemplo: Limitar local\_llm\_heavy a 1 execução por vez para não estourar VRAM/RAM local, enquanto light\_api permite 5 em paralelo.  
> 3. **Cooldown e Cadeia de Fallback de Modelos:**  
   * Se um modelo retornar erro 429 Too Many Requests ou 503, aplicar um *cooldown* temporário (ex.: 5 minutos) naquele modelo específico.  
   * Redirecionar automaticamente a tarefa para o modelo reserva configurado na cadeia de fallback (ex.: de glm-5.3 para deepseek-v4.1-flash) sem abortar a rotina.

### **2\. Máquina de Estados Heurística e "Attention Card" (Padrão *Herdr* & *Hermes*)**

> 1. **Detecção Instantânea dos 4 Estados Operacionais:**  
   * O interceptador SSE deve classificar cada agente ativo em tempo real em um de 4 estados:  
     * 🟢 IDLE (Livre)  
     * 🟡 WORKING (Gerando tokens ou executando ferramentas)  
     * 🔴 BLOCKED (Parado aguardando confirmação ask de permissão bash/edit ou resposta de esclarecimento humano)  
     * 🔵 DONE (Finalizado)  
> 2. **Attention Card no Topo do Dashboard:**  
   * Criar um componente fixo no topo da tela inicial que só aparece quando há itens críticos, exibindo exclusivamente:  
     * Agentes em estado 🔴 BLOCKED com botões de decisão rápida (\[Permitir Comando\], \[Permitir Sempre nesta Tarefa\], \[Negar\]) direto no card, sem precisar abrir o log completo.  
     * Tarefas paradas na coluna Plan aguardando aprovação para iniciar o Build.  
     * Falhas de execução nas últimas 24h com botão \[Retry com Outro Modelo\].

### **3\. Cron Resiliente e Segurança em Background (Padrão *Pi Desktop*)**

> 1. **Catch-up After Sleep (Recuperação Pós-Hibernação/Desligamento):**  
   * No boot do OpenDesk e ao detectar retorno de suspensão do sistema operacional, o cron\_engine deve comparar schedules.next\_run\_at com o relógio atual.  
   * Adicionar a propriedade misfire\_policy na rotina agendada:  
     * RUN\_IMMEDIATELY: Executa imediatamente a tarefa perdida se o PC estava desligado no horário programado.  
     * SKIP\_TO\_NEXT: Ignora a execução perdida e agenda apenas a próxima.  
> 2. **Modo "Safe Background" (Read-Only Default para Crons):**  
   * Adicionar toggle safe\_readonly\_mode nas rotinas agendadas. Quando ativo, a rotina pode ler arquivos, buscar na web e escrever **apenas** dentro da sua pasta determinística /output/YYYY-MM/\<slug\>/, bloqueando via permissão qualquer deleção ou edição no código-fonte fora da pasta de saída.  
> 3. **Dry-Run de Agendamento (Teste em 1 Clique):**  
   * Botão \[Simular Agora (Dry-Run)\] na lista de crons que valida o prompt, testa a criação da pasta e verifica a conexão SMTP/Google sem esperar o horário do agendamento.

### **4\. Gestão Avançada de Contexto e Sessão (Padrão *Pi* & *Claude Code*)**

> 1. **Medidor de Contexto e Compactação Manual/Automática (Compaction):**  
   * Exibir uma barra de preenchimento da janela de contexto em cada tarefa ativa (ex.: 42k / 128k tokens \- 32%).  
   * Botão \[Compactar Contexto Agora\] e gatilho automático ao atingir 80%: chama um modelo rápido para resumir o histórico intermediário em fatos estruturados e limpa os logs brutos de ferramentas antigas da sessão.  
> 2. **Session Branching / Forking de Tarefas:**  
   * Em qualquer etapa do histórico de uma tarefa ou card do Kanban, incluir o botão \[Criar Fork a partir daqui\].  
   * Duplica o estado e os artefatos gerados até aquele ponto exato em um novo card independente, permitindo testar outra abordagem ou outro modelo sem perder o trabalho original.  
> 3. **Memória Persistente Hierárquica (MEMORY.md Auto- Aprendizado):**  
   * Adicionar um arquivo \<workspace\>/.opencode/MEMORY.md gerenciado pelo OpenDesk.  
   * Botão \[Fixar na Memória do Workspace\] em qualquer resposta ou correção feita pelo usuário, gravando a regra aprendida em uma linha no MEMORY.md para que os agentes nunca repitam o mesmo erro naquele projeto.

### **5\. Inteligência Financeira e Observabilidade (Padrão *Hermes Workspace*)**

> 1. **Cost Ledger & Barra de "Model Mix":**  
   * Visualizador horizontal compacto mostrando a proporção percentual exata de uso por família de modelos na semana (ex.: 78% Flash \| 18% GLM \| 4% Pro).  
   * Indicador de **Economia Gerada pelo Roteador**: mostra quanto a operação custaria se 100% das tarefas tivessem rodado no modelo mais caro vs. o gasto real obtido pelo roteamento DSH.  
> 2. **Live Scratchpad & Thinking Accordion:**  
   * Na visualização de execução da tarefa, separar visualmente o bloco de raciocínio interno (\<think\> / *reasoning tokens*) em um acordeão colapsado com contador de tempo ("Pensou por 4.2s"), mantendo a tela limpa e focada apenas nas ações e resultados finais.

### **6\. Micro-Funcionalidades de Produtividade e Polimento (UX 10/10)**

> 1. **Quick Capture Global (Ctrl+Shift+Space) com Clipboard Inteligente:**  
   * Janela flutuante minimalista acionada de qualquer lugar do sistema operacional.  
   * Suporta colar prints de tela (Ctrl+V de imagem), links ou trechos de texto selecionados e despachar diretamente para um workspace/subagente sem abrir a janela principal do OpenDesk.  
> 2. **Pinning de Artefatos ("Entregáveis Oficiais"):**  
   * Na lista de artefatos criados, permitir marcar arquivos específicos com estrela (Pin as Deliverable).  
   * Arquivos fixados aparecem diretamente na capa do card do Kanban e no topo do resumo do Workspace, separando rascunhos intermediários dos documentos definitivos.  
> 3. **Rollback de Artefato em 1 Clique:**  
   * No visualizador de Diff da aba Artefatos, adicionar o botão \[Reverter esta Alteração\]. Como o OpenDesk já salva o diff\_patch na tabela artifacts\_provenance, o Rust aplica o patch reverso instantaneamente caso um agente tenha estragado uma parte do arquivo.  
> 4. **Enxame Paralelo Comparativo (*A/B Swarm Run*):**  
   * Opção no card da tarefa para **"Rodar em 2 Agentes/Modelos em Paralelo"** (ex.: disparar a mesma tarefa para deepseek-v4.1-flash e glm-5.3 em pastas isoladas) e exibir os dois arquivos gerados lado a lado para o usuário escolher o vencedor.  
> 5. **Templates de Equipe em 1 Clique (*Workspace Blueprints*):**  
   * Ao criar um novo Workspace, permitir escolher um pacote pré-configurado (ex.: *Equipe de Conteúdo/YouTube*, *Equipe de Engenharia de Software*, *Equipe de Pesquisa & Mercado*) que já popula automaticamente a pasta .opencode/agents/ e .opencode/skills/ com os especialistas padrão daquela operação.

A especificação anterior cobriu com precisão as engrenagens de backend, o SSOT no disco, o roteador DSH e os diferenciais operacionais dos concorrentes. No entanto, para que o \*\*OpenDesk\*\* atinja um padrão \*\*10/10 absoluto\*\* em experiência de uso e se alinhe perfeitamente ao seu ecossistema atual e histórico de desenvolvimento, faltaram \*\*5 pontos críticos e diferenciais de integração fina\*\*:

\---

\#\#\# 1\. Integração Direta com a Cota do OpenCode Go (Porta \`3100\`) e Cache Hit Optimization

\* \*\*O que faltou:\*\* A especificação detalhou o uso de modelos mas não pontuou a otimização de \*\*Prompt Caching\*\* e o gerenciamento de sessões para aproveitar ao máximo a assinatura do OpenCode Go.  
\* \*\*Como entra no OpenDesk:\*\*  
\* O \`context\_packer\` e o roteador DSH devem estruturar os prompts fixando os System Prompts e as Skills no início da requisição (utilizando marcadores de cache do provedor) para maximizar o \*\*Cache Hit Ratio\*\* no endpoint local \`:3100\`.  
\* Um widget de telemetria dedicado na UI para mostrar o percentual de tokens economizados graças ao cache do OpenCode Go.

\#\#\# 2\. Suporte Nativo a Codecs de Vídeo / Mídia (DaVinci Resolve / OBS Workflows)

\* \*\*O que faltou:\*\* Considerando seus projetos de mídia, canais de IA e edição de vídeo, o OpenDesk tratou apenas de arquivos textuais (\`.md\`, código, PDFs). Agentes que lidam com automação de conteúdo precisam interagir com ativos multimídia.  
\* \*\*Como entra no OpenDesk:\*\*  
\* Adicionar nas Skills padrão e ferramentas do OpenCode a capacidade de ler metadados de arquivos de vídeo/áudio e se conectar via MCP a ferramentas de automação de pós-produção (como scripts de corte do DaVinci Resolve ou organização de pastas de assets de vídeo).

\#\#\# 3\. Mecanismo de "Human-in-the-Loop" por E-mail / Telegram / WhatsApp (Notificações Interativas)

\* \*\*O que faltou:\*\* A especificação citou notificações por e-mail e bloqueio por estado 🔴 \`BLOCKED\`, mas focou apenas na interface web. Se você estiver longe do PC e um agente precisar de aprovação, ele fica travado no daemon.  
\* \*\*Como entra no OpenDesk:\*\*  
\* \*\*Aprovação via E-mail ou Webhook:\*\* Quando um agente entra em estado \`BLOCKED\` (ex.: precisa de autorização para rodar um comando bash sensível ou aprovar um plano), o OpenDesk pode disparar um e-mail formatado ou uma notificação contendo botões criptografados de ação rápida (\*\[Aprovar\]\*, \*\[Rejeitar\]\*) permitindo desbloquear a tarefa diretamente pelo celular sem abrir o app desktop.

\#\#\# 4\. Gestão e Sincronização de Credenciais / Segredos Locais (Bitwarden / Dotfiles)

\* \*\*O que faltou:\*\* Como o OpenDesk gerencia integrações com Google Workspace (OAuth2), chaves de API do DeepSeek, Amazon SES e servidores MCP, faltou definir como o sistema lida com segredos sem expô-los em texto plano no banco SQLite ou no \`opencode.jsonc\`.  
\* \*\*Como entra no OpenDesk:\*\*  
\* O OpenDesk deve se integrar nativamente ao \*\*Keyring do Sistema Operacional\*\* (GNOME Keyring / Windows Credential Manager) ou ler variáveis de ambiente seguras de arquivos \`.env\` locais por workspace, sem nunca gravar credenciais sensíveis nas tabelas do SQLite de tarefas ou logs de execução.

\#\#\# 5\. Modo "Pair-Programming / Conversa Contínua" com o Agente Principal (O Balcão de Despacho)

\* \*\*O que faltou:\*\* O foco ficou muito nos cards do Kanban e na automação em background, deixando vaga a experiência de "bater papo" quando você quer co-criar algo do zero com o modelo principal do OpenCode.  
\* \*\*Comportamento esperado:\*\*  
\* A aba \*\*Chat & Criador\*\* do OpenDesk precisa simular um terminal moderno de IA (estilo Cursor/Raycast Chat) onde você pode alternar instantaneamente entre o modelo leve (Flash) para perguntas rápidas e o modelo pesado (Pro/GLM) para debugar um erro complexo de código ou estruturar uma pauta, com arraste e solto de arquivos e imagens direto na caixa de texto.  
