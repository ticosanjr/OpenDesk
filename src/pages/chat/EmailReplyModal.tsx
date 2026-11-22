import { useState } from "react";
import { Loader2, Mail } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Field, Input } from "@/components/ui/field";
import { Modal } from "@/components/ui/modal";
import { apiPost } from "@/lib/api";
import type { ChatThread } from "@/lib/types";
import { toast } from "@/store/toast-store";
import { escapeHtml } from "./helpers";

export function EmailReplyModal({
  open,
  onClose,
  thread,
  subject,
  onSubjectChange,
}: {
  open: boolean;
  onClose: () => void;
  thread: ChatThread | null;
  subject: string;
  onSubjectChange: (value: string) => void;
}) {
  const [emailTo, setEmailTo] = useState("");
  const [emailBody, setEmailBody] = useState("");
  const [emailAttachments, setEmailAttachments] = useState("");
  const [emailSending, setEmailSending] = useState(false);

  async function sendEmailReply() {
    if (!thread) return;
    if (!emailTo.trim()) {
      toast({ tone: "warning", title: "Informe o destinatário" });
      return;
    }
    setEmailSending(true);
    try {
      await apiPost(`/api/chat/threads/${encodeURIComponent(thread.id)}/email-reply`, {
        to: emailTo.trim(),
        subject: subject.trim() || `Re: ${thread.title}`,
        body_html: emailBody.trim()
          ? `<pre style="white-space:pre-wrap">${escapeHtml(emailBody)}</pre>`
          : undefined,
        attachments: emailAttachments
          .split(",")
          .map((value) => value.trim())
          .filter(Boolean),
      });
      toast({ tone: "success", title: "Resposta enviada" });
      onClose();
      setEmailBody("");
      setEmailAttachments("");
    } catch (cause) {
      toast({
        tone: "error",
        title: "Envio falhou",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setEmailSending(false);
    }
  }

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Responder por e-mail"
      description="A resposta mantém os headers In-Reply-To/References da thread."
    >
      <div className="space-y-3">
        <Field label="Para" htmlFor="email-to">
          <Input
            id="email-to"
            value={emailTo}
            onChange={(event) => setEmailTo(event.target.value)}
            placeholder="destinatario@exemplo.com"
          />
        </Field>
        <Field label="Assunto" htmlFor="email-subject">
          <Input
            id="email-subject"
            value={subject}
            onChange={(event) => onSubjectChange(event.target.value)}
          />
        </Field>
        <Field
          label="Mensagem"
          htmlFor="email-body"
          hint="Deixe vazio para enviar a última resposta do assistente."
        >
          <textarea
            id="email-body"
            value={emailBody}
            onChange={(event) => setEmailBody(event.target.value)}
            className="min-h-[120px] w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-2 text-[13px] text-zinc-100 outline-none focus:border-blue-500/60"
          />
        </Field>
        <Field
          label="Anexos"
          htmlFor="email-attachments"
          hint="Caminhos relativos ao workspace, separados por vírgula."
        >
          <Input
            id="email-attachments"
            value={emailAttachments}
            onChange={(event) => setEmailAttachments(event.target.value)}
            placeholder="output/2026-11/relatorio.pdf"
          />
        </Field>
        <div className="flex justify-end gap-2 border-t border-zinc-800/70 pt-3">
          <Button variant="ghost" size="sm" onClick={onClose}>
            Cancelar
          </Button>
          <Button
            size="sm"
            onClick={() => void sendEmailReply()}
            disabled={emailSending}
          >
            {emailSending ? <Loader2 size={13} className="animate-spin" /> : <Mail size={13} />}
            Enviar
          </Button>
        </div>
      </div>
    </Modal>
  );
}
