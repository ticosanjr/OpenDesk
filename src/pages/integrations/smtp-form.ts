export interface SmtpForm {
  host: string;
  port: number;
  username: string;
  fromEmail: string;
  useTls: boolean;
  password: string;
}

export const EMPTY_FORM: SmtpForm = {
  host: "",
  port: 587,
  username: "",
  fromEmail: "",
  useTls: true,
  password: "",
};
