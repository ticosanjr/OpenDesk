# 0149 — D1.2: TLS mútuo no servidor

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** a validação de CA/cert/chave/SAN existia, mas o servidor Axum
  ainda falava HTTP puro.

## Decisão

1. Quando `OPENDESK_MTLS_CA`, `OPENDESK_MTLS_CERT` e `OPENDESK_MTLS_KEY` estão
   definidos, o listener da API sobe com TLS mútuo.
2. `WebPkiClientVerifier` exige certificado de cliente assinado pela CA.
3. O material é validado por `mtls::validate` no boot (SAN, expiração e
   assinatura).
4. `Reloader` verifica os arquivos a cada 5s e reconstrói `ServerConfig` sem
   reiniciar o listener. Conexões em andamento mantêm a configuração antiga;
   novas conexões usam a nova.
5. O servidor usa `tokio-rustls` + `hyper-util`; HTTP/1, SSE e WebSocket
   continuam funcionando.
6. Configuração parcial de env é erro, não fallback silencioso para HTTP.

## Consequências

- Rotação de certificado não derruba jobs em voo.
- Quem expõe a API para nós precisa distribuir um certificado de cliente.
- O desktop local continua usando HTTP quando o mTLS não está configurado.
- Teste gera CA/cert via OpenSSL e prova que a configuração é recarregada.
