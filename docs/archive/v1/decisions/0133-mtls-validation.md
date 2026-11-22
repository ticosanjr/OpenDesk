# 0133 — D1: validação de material mTLS

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** o protocolo de nós ainda usa Bearer token; faltava uma base
  verificável para CA/cert/chave antes de habilitar TLS mútuo.

## Decisão

1. Novo `opendesk_core::mtls` valida:
   - existência de CA, cert e key;
   - permissão `0600` da chave em Unix;
   - assinatura do cert contra a CA (`openssl verify`);
   - expiração (`openssl x509 -checkend 0`);
   - presença de SAN DNS/IP.
2. `from_env` lê `OPENDESK_MTLS_CA`, `OPENDESK_MTLS_CERT`, `OPENDESK_MTLS_KEY`.
3. `doctor --live` ganha probe `mtls` (`ok`, `error`, `not-configured`).
4. Teste gera CA e cert de nó via OpenSSL e valida o material.

## Consequências

- Enrollment quebrado falha antes de o worker iniciar.
- O servidor Axum ainda não negocia TLS; isso fica em D1.2, junto com rotação.
