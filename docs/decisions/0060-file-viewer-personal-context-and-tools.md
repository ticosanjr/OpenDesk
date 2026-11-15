# 0060 — Prévia HTML/JSON, contexto pessoal e ferramentas detectadas

- **Status:** aceito
- **Data:** 2026-11-15
- **Contexto:** a prévia HTML não recarregava ao liberar scripts e não tinha
  botão de reload; JSON aparecia como texto cru; o contexto pessoal exigia um
  título desnecessário; as ferramentas externas não apareciam no Dashboard e a
  detecção dependia só do PATH.

## Decisão

1. **Prévia HTML.** O `iframe` recebe uma `key` que inclui o estado de scripts e
   um contador de reload. Liberar scripts recria o documento imediatamente e o
   botão **Recarregar** repete a carga. A preferência é salva em
   `html_allow_scripts`.
2. **JSON formatado.** Arquivos `json`, `jsonc` e `jsonl` ganham visualização
   dedicada com `JSON.stringify(..., null, 2)`, linha a linha para JSONL. JSON
   inválido mantém o texto original com aviso; a aba “fonte” continua mostrando
   os bytes originais. Sem dependência de Prettier.
3. **Contexto pessoal sem título.** A UI mostra apenas a regra e o escopo. O
   campo `name` da API vira opcional; quando ausente, o backend deriva um rótulo
   curto da primeira linha para uso interno.
4. **Ferramentas externas.** A detecção procura aliases (`whisper-cli`,
   `whisper-cpp`, `whisper`), o PATH e diretórios comuns (`/usr/bin`,
   `/usr/local/bin`, `/snap/bin`, `~/.local/bin`, Homebrew etc.), além do
   caminho manual e do binário gerenciado. O Dashboard ganha um item
   “Ferramentas” com `instaladas/total` e link para Configurações → Ferramentas.
5. **Sem instalação privilegiada.** Nada de `pkexec`/gestor de pacotes: o painel
   detecta, aponta o caminho e mantém o download gerenciado do Typst; Tesseract,
   Whisper e ffprobe continuam com instalação do sistema indicada.

## Consequências

- Scripts liberados passam a valer imediatamente, sem fechar e reabrir o arquivo.
- JSON fica legível sem inflar o bundle com um formatador.
- Regras pessoais ficam mais diretas e o prompt mostra só o texto útil.
- O usuário vê rapidamente quais ferramentas externas estão realmente
  disponíveis, sem o app pedir privilégios.
