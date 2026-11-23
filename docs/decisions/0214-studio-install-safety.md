# 0214 — Instalação do Studio fail-closed (SSRF, ZIP e symlink)

- **Status:** aceito
- **Data:** 2026-11-23
- **Contexto:** `studio_ops_install` baixava URLs sem validar o destino, lia o
  corpo inteiro em memória, seguia redirects sem revalidar e extraía ZIP com
  `name().contains("..")`. O limite de 20 MB era aplicado só depois do download;
  não havia limite total descompactado nem bloqueio de symlink.

## Decisão

1. `validate_external_url` permite apenas `https`; em testes, `http` é aceito
   somente para loopback. Resolve o host com `tokio::net::lookup_host` e rejeita
   loopback, RFC1918, link-local, ULA IPv6, IPv4 mapeado e `0.0.0.0/8`.
2. Downloads usam `reqwest` com `redirect::Policy::none()` e até 5 redirects
   manuais; cada `Location` passa novamente por `validate_external_url`. O token
   do GitHub só é enviado para `api.github.com`.
3. O corpo é lido em chunks com teto de **25 MiB**; `Content-Length` acima do
   teto aborta antes de alocar.
4. ZIPs: `SafeArchiveLimits` (250 entradas, 50 MiB descompactados, ratio 50)
   com `enclosed_name()` obrigatório, symlinks ignorados e entrada individual
   até 2 MiB. A leitura usa `Read::take(restante + 1)` antes de materializar o
   conteúdo; o mesmo struct limita downloads e diretórios locais.
5. Diretórios locais são canonicalizados; symlinks e arquivos que resolvem para
   fora do root são ignorados, com limite de 250 arquivos e 50 MiB.
6. `PermissionsExt` foi auditado em `api/updates.rs`, `updates.rs` e
   `io_bus/tools.rs`: todos os usos já estão dentro de `#[cfg(unix)]`; não há
   fallback de chmod em plataformas sem bits Unix.
7. O código foi separado em `api/studio_install_safety.rs` (URL/ZIP/local) e
   `api/studio_github.rs` (resolução de repo/token), mantendo os arquivos abaixo
   de 500 linhas.

## Consequências

- Fontes remotas em HTTP, IP privado ou metadata cloud falham com 400 antes de
  qualquer conexão.
- ZIP malicioso não escapa do diretório, não cria symlink e não expande sem
  limite.
- Testes cobrem IPs privados/metadata, redirect para IP privado, zip-slip,
  excesso de entradas, bomba de compressão, arquivo vazio e symlink local.
- Instalações legítimas de GitHub/https continuam funcionando; redirecionamentos
  oficiais do GitHub são revalidados.
