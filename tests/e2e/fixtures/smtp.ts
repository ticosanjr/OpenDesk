import { createServer, type Server, type Socket } from "node:net";

export interface FakeSmtp {
  port: number;
  messages: string[];
  close: () => Promise<void>;
}

/** Minimal SMTP server for E2E tests: EHLO/MAIL/RCPT/DATA/QUIT. */
export async function startFakeSmtp(port = 2525): Promise<FakeSmtp> {
  const messages: string[] = [];
  const server: Server = createServer((socket: Socket) => {
    let buffer = "";
    let inData = false;
    let message = "";
    socket.write("220 opendesk-e2e ESMTP\r\n");
    socket.on("data", (chunk) => {
      buffer += chunk.toString("utf8");
      let index = buffer.indexOf("\r\n");
      while (index >= 0) {
        const line = buffer.slice(0, index);
        buffer = buffer.slice(index + 2);
        if (inData) {
          if (line === ".") {
            inData = false;
            messages.push(message);
            message = "";
            socket.write("250 2.0.0 queued\r\n");
          } else {
            message += `${line}\n`;
          }
        } else {
          const command = line.toUpperCase();
          if (command.startsWith("EHLO") || command.startsWith("HELO")) {
            socket.write("250-opendesk-e2e\r\n250-8BITMIME\r\n250 OK\r\n");
          } else if (command.startsWith("MAIL FROM") || command.startsWith("RCPT TO")) {
            socket.write("250 2.1.0 OK\r\n");
          } else if (command === "DATA") {
            inData = true;
            socket.write("354 End data with <CR><LF>.<CR><LF>\r\n");
          } else if (command === "QUIT") {
            socket.write("221 2.0.0 Bye\r\n");
            socket.end();
          } else if (command === "RSET" || command === "NOOP") {
            socket.write("250 OK\r\n");
          } else {
            socket.write("235 2.7.0 Authentication successful\r\n");
          }
        }
        index = buffer.indexOf("\r\n");
      }
    });
  });
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => resolve());
  });
  return {
    port,
    messages,
    close: () =>
      new Promise((resolve) => {
        server.close(() => resolve());
      }),
  };
}
