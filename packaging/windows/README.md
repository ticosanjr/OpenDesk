# opendeskd no Windows

O binário `opendeskd.exe` roda em primeiro plano e aceita CTRL+C. Para serviço,
use o [WinSW](https://github.com/winsw/winsw) ou `sc.exe` com um wrapper.

Exemplo de configuração WinSW:

```xml
<service>
  <id>opendeskd</id>
  <name>OpenDesk control plane</name>
  <executable>%LOCALAPPDATA%\OpenDesk\opendeskd.exe</executable>
  <env name="OPENDESK_API_HOST" value="127.0.0.1"/>
  <onfailure action="restart" delay="3 sec"/>
</service>
```

Para acesso remoto, mantenha o bind em `127.0.0.1` e use Tailscale, WireGuard ou
um reverse proxy HTTPS com autenticação.
