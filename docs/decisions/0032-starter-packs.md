# 0032 — Pacotes iniciais opcionais

- **Status:** aceito
- **Data:** 2026-11-08
- **Contexto:** o OpenDesk abria vazio. Criar agentes, skills, receitas e
  rotinas do zero é lento e afasta usuários casuais. Ao mesmo tempo, instalar
  conteúdo automaticamente violaria o princípio de SSOT e surpreenderia quem
  já tem configuração própria.
- **Decisão:**
  1. Pacotes são um bundle embutido (`starter-packs/`) com manifesto, agentes,
     skills, comandos, receitas e rotinas. Nada é instalado no primeiro boot.
  2. A instalação é explícita, com prévia, detecção de conflito, backup e
     checksums. Reinstalar é idempotente.
  3. A remoção só apaga arquivos não modificados; rotinas já executadas ou
     ativas são mantidas junto com sua receita.
  4. Rotinas de pacote entram inativas. Receitas com efeito externo terminam
     em HITL.
  5. Templates de workspace apenas orquestram a criação do workspace e a
     instalação de pacotes; o usuário pode começar vazio.
  6. A proveniência fica em `starter_pack_installs` (SQLite), enquanto a
     configuração continua no disco do workspace.
- **Consequências:** migração `0022`; nova página Pacotes; API `/api/starter-packs`;
  catálogo inicial com 24 pacotes e 6 templates; testes de bundle, colisão,
  instalação, remoção e template.
